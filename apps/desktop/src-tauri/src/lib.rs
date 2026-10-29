//! SAYL Desktop Tauri backend.
//!
//! Orchestrates the `sayl-app` download engine, manages settings,
//! and communicates with the Svelte frontend via Tauri commands and events.

pub mod autostart;
pub mod browsers;
pub mod capture_diagnostics;
pub mod capture_server;
pub mod clipboard;
pub mod completion;
pub mod delete;
pub mod duplicates;
pub mod native_host;
pub mod perf;
pub mod power;
pub mod shutdown;
pub mod tray;

pub use browsers::{BrowserStatus, SUPPORTED_BROWSERS};
pub use delete::{execute_delete_tasks, send_to_recycle_bin};
pub use duplicates::{find_duplicate, normalize_url_for_compare};
pub use native_host::{NativeHostPing, NativeHostStatus, RegisteredStatus};
pub use power::{should_inhibit, SleepInhibitor};
pub use shutdown::{
    shutdown_decision, ShutdownController, ShutdownDecision, ShutdownRunner, ShutdownStatus,
    SHUTDOWN_COUNTDOWN_SECS,
};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use sayl_app::{Engine, EngineConfig};
use sayl_core::id::TaskId;
use sayl_core::state::DownloadState;
use sayl_storage::TaskRecord;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};

/// Live stats for active tasks, tracking instantaneous transfer rate and ETA.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LiveTaskStats {
    pub speed_bps: u64,
    pub eta_secs: Option<u64>,
}

fn default_close_action() -> String {
    "background".to_string()
}

fn default_true() -> bool {
    true
}

/// User configurable settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Settings {
    pub download_dir: String,
    pub max_concurrent_tasks: usize,
    pub max_connections_per_task: usize,
    pub theme: String,
    pub language: String,
    pub view_mode: String,
    pub sidebar_collapsed: bool,
    pub prevent_sleep: bool,
    pub show_download_window: bool,
    #[serde(default = "default_true")]
    pub show_download_complete_window: bool,
    pub close_download_window_on_finish: bool,
    #[serde(default = "default_close_action")]
    pub close_action: String,
    #[serde(default)]
    pub run_at_startup: bool,
    #[serde(default)]
    pub close_notice_shown: bool,
    #[serde(default = "default_true")]
    pub clipboard_monitor: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_dir: default_download_dir().to_string_lossy().to_string(),
            max_concurrent_tasks: 3,
            max_connections_per_task: 4,
            theme: "system".to_string(),
            language: "ar".to_string(),
            view_mode: "table".to_string(),
            sidebar_collapsed: false,
            prevent_sleep: true,
            show_download_window: true,
            show_download_complete_window: true,
            close_download_window_on_finish: false,
            close_action: "background".to_string(),
            run_at_startup: false,
            close_notice_shown: false,
            clipboard_monitor: true,
        }
    }
}

/// Validates a download directory path:
/// - Must not be empty
/// - Must be an absolute path
/// - Must not be a bare drive root
/// - Must not contain `..` or `.` components
/// - Must not contain empty path components (e.g. repeated slashes)
pub fn validate_download_dir(dir: &str) -> Result<(), String> {
    let trimmed = dir.trim();
    if trimmed.is_empty() {
        return Err("Download directory cannot be empty".to_string());
    }

    let path = std::path::Path::new(trimmed);
    if !path.is_absolute() {
        return Err("Download directory must be an absolute path".to_string());
    }

    // Bare drive root check: must have at least one normal path component
    let normal_components_count = path
        .components()
        .filter(|c| matches!(c, std::path::Component::Normal(_)))
        .count();
    if normal_components_count == 0 {
        return Err("Download directory cannot be a bare drive root".to_string());
    }

    // Reject '..' and '.' components
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err("Download directory cannot contain '..' components".to_string());
            }
            std::path::Component::CurDir => {
                return Err("Download directory cannot contain '.' components".to_string());
            }
            _ => {}
        }
    }

    // Reject empty path components (e.g. repeated separators "//" or "\\")
    let check_str = trimmed.strip_prefix(r"\\").unwrap_or(trimmed);
    if check_str.contains("//")
        || check_str.contains(r"\\")
        || check_str.contains(r"/\")
        || check_str.contains(r"\/")
    {
        return Err("Download directory cannot contain empty path components".to_string());
    }

    Ok(())
}

/// Validates settings bounds and allowed enum values.
pub fn validate_settings(settings: &Settings) -> Result<(), String> {
    validate_download_dir(&settings.download_dir)?;
    if settings.max_concurrent_tasks == 0 || settings.max_concurrent_tasks > 32 {
        return Err("Concurrent tasks must be between 1 and 32".to_string());
    }
    if settings.max_connections_per_task == 0 || settings.max_connections_per_task > 64 {
        return Err("Connections per task must be between 1 and 64".to_string());
    }
    if !["system", "dark", "light"].contains(&settings.theme.as_str()) {
        return Err("Theme must be system, dark, or light".to_string());
    }
    if !["ar", "en"].contains(&settings.language.as_str()) {
        return Err("Language must be ar or en".to_string());
    }
    if !["cards", "table"].contains(&settings.view_mode.as_str()) {
        return Err("View mode must be cards or table".to_string());
    }
    if !["background", "quit"].contains(&settings.close_action.as_str()) {
        return Err("Close action must be background or quit".to_string());
    }
    Ok(())
}

/// Active worker count estimate: for Downloading use min(parts, settings.max_connections_per_task) else 1/None.
///
/// Approximation rationale: In `sayl-app`, tasks in `Downloading` state allocate workers up to the number of
/// available parts or the task connection cap (`settings.max_connections_per_task`). During `Probing`, a single HTTP
/// connection is active to query headers. In non-running states (Queued, Paused, Completed, Cancelled, Error), no
/// worker connections are active.
pub fn estimate_connections(
    state: DownloadState,
    parts: Option<u32>,
    max_connections_per_task: usize,
) -> Option<u32> {
    match state {
        DownloadState::Downloading => {
            let p = parts.unwrap_or(1);
            let limit = (max_connections_per_task as u32).max(1);
            Some(p.min(limit))
        }
        DownloadState::Probing => Some(1),
        _ => None,
    }
}

/// Flattened task representation sent to the Svelte frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskView {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub state: String,
    pub total_size: Option<u64>,
    pub written: u64,
    pub speed_bps: Option<u64>,
    pub eta_secs: Option<u64>,
    pub error: Option<String>,
    pub mime: Option<String>,
    pub parts: Option<u32>,
    #[serde(default)]
    pub error_kind: Option<String>,
    #[serde(default)]
    pub connections: Option<u32>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub folder: String,
    /// Per-task speed cap in KB/s; `None` means unlimited.
    #[serde(default)]
    pub speed_limit_kbps: Option<u32>,
}

impl TaskView {
    /// Constructs a `TaskView` with default connection estimate.
    pub fn from_record(
        rec: &TaskRecord,
        speed_bps: Option<u64>,
        eta_secs: Option<u64>,
        parts: Option<u32>,
    ) -> Self {
        Self::from_record_full(rec, speed_bps, eta_secs, parts, None)
    }

    /// Constructs a `TaskView` from a durable `TaskRecord` and transient stats/connections.
    pub fn from_record_full(
        rec: &TaskRecord,
        speed_bps: Option<u64>,
        eta_secs: Option<u64>,
        parts: Option<u32>,
        connections: Option<u32>,
    ) -> Self {
        let is_running =
            rec.state == DownloadState::Downloading || rec.state == DownloadState::Probing;
        let completed_at = if rec.state == DownloadState::Completed {
            Some(rec.updated_at.to_rfc3339())
        } else {
            None
        };
        Self {
            id: rec.id.to_string(),
            url: rec.url.clone(),
            filename: rec.filename.clone(),
            state: rec.state.as_str().to_string(),
            total_size: rec.total_size,
            written: rec.written,
            speed_bps: if is_running { speed_bps } else { None },
            eta_secs: if is_running { eta_secs } else { None },
            error: rec.error.clone(),
            mime: rec.content_type.clone(),
            parts,
            error_kind: rec.error_kind.clone(),
            connections,
            created_at: Some(rec.created_at.to_rfc3339()),
            completed_at,
            folder: rec.save_path.to_string_lossy().into_owned(),
            speed_limit_kbps: rec.speed_limit_kbps,
        }
    }
}

/// Payload matching the coalesced `sayl://tasks-changed` event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TasksChangedPayload {
    pub revision: u64,
    pub full: bool,
    pub upsert: Vec<TaskView>,
    pub removed: Vec<String>,
}

/// Pure helper to build a coalesced batch payload with deduplication.
pub fn build_tasks_changed_batch(
    revision: u64,
    full: bool,
    pending_upsert_ids: &[TaskId],
    pending_removed_ids: &[String],
    lookup_view: impl Fn(TaskId) -> Option<TaskView>,
    full_views: impl Fn() -> Vec<TaskView>,
) -> TasksChangedPayload {
    if full {
        let upsert = full_views();
        TasksChangedPayload {
            revision,
            full: true,
            upsert,
            removed: Vec::new(),
        }
    } else {
        let mut deduped_ids = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for &id in pending_upsert_ids {
            if seen.insert(id) {
                deduped_ids.push(id);
            }
        }
        let upsert = deduped_ids.into_iter().filter_map(lookup_view).collect();
        let mut removed = Vec::new();
        let mut seen_rem = std::collections::HashSet::new();
        for id in pending_removed_ids {
            if seen_rem.insert(id.clone()) {
                removed.push(id.clone());
            }
        }
        TasksChangedPayload {
            revision,
            full: false,
            upsert,
            removed,
        }
    }
}

/// Outcome of invoking `add_task`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AddTaskOutcome {
    Added { task: TaskView },
    Duplicate { existing: TaskView },
}

/// Progress event payload matching `sayl://progress`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProgressPayload {
    pub id: String,
    pub written: u64,
    pub total: Option<u64>,
    pub speed_bps: u64,
}

/// Shared application state managed by Tauri.
pub struct AppState {
    pub engine: Engine,
    pub settings: RwLock<Settings>,
    pub live_stats: Arc<Mutex<HashMap<TaskId, LiveTaskStats>>>,
    pub parts_cache: Arc<RwLock<HashMap<TaskId, Option<u32>>>>,
    pub active_count: Arc<AtomicUsize>,
    /// Monotonic revision shared by every `sayl://tasks-changed` emitter.
    pub tasks_revision: Arc<AtomicU64>,
    pub settings_path: PathBuf,
    pub sleep_inhibitor: Arc<SleepInhibitor>,
    pub shutdown_controller: Arc<ShutdownController>,
    pub clipboard_monitor: Arc<AtomicBool>,
}

/// Revision used by the first (full) `sayl://tasks-changed` emission.
pub const INITIAL_TASKS_REVISION: u64 = 1;

/// Allocates the next monotonic `sayl://tasks-changed` revision.
///
/// Every emitter (coalescing pump, delete command) shares one counter so the
/// webview never sees a stale or repeated revision, and never a `0`.
pub fn next_tasks_revision(counter: &AtomicU64) -> u64 {
    counter.fetch_add(1, Ordering::Relaxed) + 1
}

/// Builds the `sayl://tasks-changed` payload announcing removed tasks.
pub fn build_removed_tasks_changed_payload(
    revision: u64,
    removed: Vec<String>,
) -> TasksChangedPayload {
    TasksChangedPayload {
        revision,
        full: false,
        upsert: Vec::new(),
        removed,
    }
}

impl AppState {
    pub fn new(
        engine: Engine,
        settings: Settings,
        settings_path: PathBuf,
        sleep_inhibitor: Arc<SleepInhibitor>,
        shutdown_controller: Arc<ShutdownController>,
    ) -> Self {
        let clipboard_monitor = Arc::new(AtomicBool::new(settings.clipboard_monitor));
        Self {
            engine,
            settings: RwLock::new(settings),
            live_stats: Arc::new(Mutex::new(HashMap::new())),
            parts_cache: Arc::new(RwLock::new(HashMap::new())),
            active_count: Arc::new(AtomicUsize::new(0)),
            tasks_revision: Arc::new(AtomicU64::new(INITIAL_TASKS_REVISION)),
            settings_path,
            sleep_inhibitor,
            shutdown_controller,
            clipboard_monitor,
        }
    }

    /// Allocates the next revision for a `sayl://tasks-changed` payload.
    pub fn next_tasks_revision(&self) -> u64 {
        next_tasks_revision(&self.tasks_revision)
    }
}

/// Default download directory resolution.
pub fn default_download_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SAYL_DOWNLOAD_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(user_dirs) = std::env::var_os("USERPROFILE") {
        let downloads = PathBuf::from(user_dirs).join("Downloads");
        if downloads.exists() {
            return downloads;
        }
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("downloads")
}

/// Default data directory for SAYL database and config.
pub fn default_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SAYL_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(app_data) = std::env::var_os("APPDATA") {
        let candidate = PathBuf::from(app_data).join("SAYL");
        if std::fs::create_dir_all(&candidate).is_ok() {
            return candidate;
        }
    }
    let local = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".sayl");
    if std::fs::create_dir_all(&local).is_ok() {
        return local;
    }
    std::env::temp_dir().join("sayl-data")
}

pub fn is_active_state(state: DownloadState) -> bool {
    matches!(
        state,
        DownloadState::Queued
            | DownloadState::Probing
            | DownloadState::Downloading
            | DownloadState::Verifying
            | DownloadState::Finalizing
    )
}

pub(crate) fn update_sleep_inhibition_with_count(
    settings: &RwLock<Settings>,
    inhibitor: &SleepInhibitor,
    active_count: usize,
) {
    let prevent_sleep = settings.read().map(|s| s.prevent_sleep).unwrap_or(true);
    inhibitor.set_active(power::should_inhibit(prevent_sleep, active_count));
}

#[allow(dead_code)]
pub(crate) fn update_sleep_inhibition(
    engine: &Engine,
    settings: &RwLock<Settings>,
    inhibitor: &SleepInhibitor,
) {
    let prevent_sleep = settings.read().map(|s| s.prevent_sleep).unwrap_or(true);
    let active_count = match engine.list_tasks() {
        Ok(tasks) => tasks
            .into_iter()
            .filter(|t| is_active_state(t.state))
            .count(),
        Err(_) => 0,
    };
    inhibitor.set_active(power::should_inhibit(prevent_sleep, active_count));
}

pub(crate) fn task_parts_count(engine: &Engine, task_id: TaskId) -> Option<u32> {
    engine
        .load_checkpoint(task_id)
        .ok()
        .flatten()
        .and_then(|cp| {
            if cp.range_map.is_empty() {
                None
            } else {
                Some(cp.range_map.len() as u32)
            }
        })
}

pub(crate) fn get_cached_parts(
    engine: &Engine,
    parts_cache: &RwLock<HashMap<TaskId, Option<u32>>>,
    task_id: TaskId,
) -> Option<u32> {
    if let Ok(cache) = parts_cache.read() {
        if let Some(&parts) = cache.get(&task_id) {
            return parts;
        }
    }

    let parts = task_parts_count(engine, task_id);
    if let Ok(mut cache) = parts_cache.write() {
        cache.insert(task_id, parts);
    }
    parts
}

// ----------------------------------------------------------------------------
// Tauri Commands
// ----------------------------------------------------------------------------

pub mod commands {
    use super::*;

    #[tauri::command]
    pub async fn add_task(
        state: State<'_, AppState>,
        url: String,
        force: Option<bool>,
    ) -> Result<AddTaskOutcome, String> {
        let clean_url = url.trim();
        if clean_url.is_empty() {
            return Err("URL cannot be empty".to_string());
        }

        let max_conn = state
            .settings
            .read()
            .map(|s| s.max_connections_per_task)
            .unwrap_or(4);

        if force != Some(true) {
            let records = state
                .engine
                .list_tasks()
                .map_err(|e| format!("Storage read failed: {e}"))?;
            if let Some(existing_rec) = duplicates::find_duplicate(&records, clean_url) {
                let stats_guard = state.live_stats.lock().ok();
                let stats = stats_guard
                    .as_ref()
                    .and_then(|m| m.get(&existing_rec.id).copied());
                let parts = get_cached_parts(&state.engine, &state.parts_cache, existing_rec.id);
                let connections = estimate_connections(existing_rec.state, parts, max_conn);
                let view = TaskView::from_record_full(
                    existing_rec,
                    stats.map(|s| s.speed_bps),
                    stats.and_then(|s| s.eta_secs),
                    parts,
                    connections,
                );
                return Ok(AddTaskOutcome::Duplicate { existing: view });
            }
        }

        let task_id = state
            .engine
            .add(clean_url)
            .await
            .map_err(|e| format!("Failed to add task: {e}"))?;

        // Auto-start the newly added task
        state.engine.spawn_start(task_id);
        let active = state.active_count.fetch_add(1, Ordering::Relaxed) + 1;
        update_sleep_inhibition_with_count(&state.settings, &state.sleep_inhibitor, active);

        let record = state
            .engine
            .get_task(task_id)
            .map_err(|e| format!("Storage read failed: {e}"))?
            .ok_or_else(|| "Task record not found after insert".to_string())?;

        let parts = get_cached_parts(&state.engine, &state.parts_cache, task_id);
        let connections = estimate_connections(record.state, parts, max_conn);

        Ok(AddTaskOutcome::Added {
            task: TaskView::from_record_full(&record, None, None, parts, connections),
        })
    }

    /// Adds a task and leaves it `Queued` so the "Download File Info" window can
    /// show the probed filename/size before any bytes are downloaded.
    ///
    /// Mirrors `add_task` (same duplicate check) but skips `spawn_start`; the
    /// caller starts it later with `start_task` once the user confirms.
    #[tauri::command]
    pub async fn add_task_queued(
        state: State<'_, AppState>,
        url: String,
        force: Option<bool>,
    ) -> Result<AddTaskOutcome, String> {
        let clean_url = url.trim();
        if clean_url.is_empty() {
            return Err("URL cannot be empty".to_string());
        }

        let max_conn = state
            .settings
            .read()
            .map(|s| s.max_connections_per_task)
            .unwrap_or(4);

        if force != Some(true) {
            let records = state
                .engine
                .list_tasks()
                .map_err(|e| format!("Storage read failed: {e}"))?;
            if let Some(existing_rec) = duplicates::find_duplicate(&records, clean_url) {
                let stats_guard = state.live_stats.lock().ok();
                let stats = stats_guard
                    .as_ref()
                    .and_then(|m| m.get(&existing_rec.id).copied());
                let parts = get_cached_parts(&state.engine, &state.parts_cache, existing_rec.id);
                let connections = estimate_connections(existing_rec.state, parts, max_conn);
                let view = TaskView::from_record_full(
                    existing_rec,
                    stats.map(|s| s.speed_bps),
                    stats.and_then(|s| s.eta_secs),
                    parts,
                    connections,
                );
                return Ok(AddTaskOutcome::Duplicate { existing: view });
            }
        }

        let task_id = state
            .engine
            .add(clean_url)
            .await
            .map_err(|e| format!("Failed to add task: {e}"))?;

        let record = state
            .engine
            .get_task(task_id)
            .map_err(|e| format!("Storage read failed: {e}"))?
            .ok_or_else(|| "Task record not found after insert".to_string())?;

        let parts = get_cached_parts(&state.engine, &state.parts_cache, task_id);
        let connections = estimate_connections(record.state, parts, max_conn);

        Ok(AddTaskOutcome::Added {
            task: TaskView::from_record_full(&record, None, None, parts, connections),
        })
    }

    #[tauri::command]
    pub async fn prepare_capture_task(
        state: State<'_, AppState>,
        id: String,
        filename: String,
        save_path: String,
    ) -> Result<(), String> {
        validate_download_dir(&save_path)?;
        let task_id = id
            .parse::<TaskId>()
            .map_err(|e| format!("Invalid task ID: {e}"))?;
        state
            .engine
            .update_queued_destination(task_id, PathBuf::from(save_path), filename)
            .map_err(|e| e.to_string())
    }

    #[tauri::command]
    pub async fn extension_diagnostics() -> Result<capture_diagnostics::ExtensionDiagnostics, String>
    {
        Ok(capture_diagnostics::collect_extension_diagnostics().await)
    }

    #[tauri::command]
    pub async fn start_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
        let task_id = id
            .parse::<TaskId>()
            .map_err(|e| format!("Invalid task ID: {e}"))?;

        state.engine.spawn_start(task_id);
        let active = state.active_count.load(Ordering::Relaxed);
        update_sleep_inhibition_with_count(&state.settings, &state.sleep_inhibitor, active);
        Ok(())
    }

    #[tauri::command]
    pub async fn pause_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
        let task_id = id
            .parse::<TaskId>()
            .map_err(|e| format!("Invalid task ID: {e}"))?;

        state
            .engine
            .pause(task_id)
            .await
            .map_err(|e| format!("Failed to pause task: {e}"))?;

        if let Ok(mut stats) = state.live_stats.lock() {
            stats.remove(&task_id);
        }
        let active = state.active_count.load(Ordering::Relaxed);
        update_sleep_inhibition_with_count(&state.settings, &state.sleep_inhibitor, active);

        Ok(())
    }

    #[tauri::command]
    pub async fn cancel_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
        let task_id = id
            .parse::<TaskId>()
            .map_err(|e| format!("Invalid task ID: {e}"))?;

        state
            .engine
            .cancel(task_id)
            .await
            .map_err(|e| format!("Failed to cancel task: {e}"))?;

        if let Ok(mut stats) = state.live_stats.lock() {
            stats.remove(&task_id);
        }
        let active = state.active_count.load(Ordering::Relaxed);
        update_sleep_inhibition_with_count(&state.settings, &state.sleep_inhibitor, active);

        Ok(())
    }

    #[tauri::command]
    pub async fn set_task_speed_limit(
        state: State<'_, AppState>,
        id: String,
        kbps: Option<u32>,
    ) -> Result<(), String> {
        let task_id = id
            .parse::<TaskId>()
            .map_err(|e| format!("Invalid task ID: {e}"))?;

        state
            .engine
            .set_speed_limit(task_id, kbps)
            .map_err(|e| format!("Failed to set speed limit: {e}"))?;

        Ok(())
    }

    #[tauri::command]
    pub async fn delete_tasks(
        app_handle: tauri::AppHandle,
        state: State<'_, AppState>,
        ids: Vec<String>,
        delete_files: Option<bool>,
    ) -> Result<Vec<String>, String> {
        let task_ids: Vec<TaskId> = ids
            .iter()
            .map(|s| {
                s.parse::<TaskId>()
                    .map_err(|e| format!("Invalid task ID '{s}': {e}"))
            })
            .collect::<Result<_, _>>()?;

        let should_delete_files = delete_files.unwrap_or(false);

        let deleted =
            crate::delete::execute_delete_tasks(&state.engine, &task_ids, should_delete_files)
                .await?;

        if let Ok(mut stats) = state.live_stats.lock() {
            for id in &deleted {
                stats.remove(id);
            }
        }
        if let Ok(mut cache) = state.parts_cache.write() {
            for id in &deleted {
                cache.remove(id);
            }
        }

        let active = state.active_count.load(Ordering::Relaxed);
        update_sleep_inhibition_with_count(&state.settings, &state.sleep_inhibitor, active);

        let deleted_str_ids: Vec<String> = deleted.into_iter().map(|id| id.to_string()).collect();

        // Real revision from the shared counter: a deletion is one more change to
        // the task list, so it must never announce revision 0.
        let payload = build_removed_tasks_changed_payload(
            state.next_tasks_revision(),
            deleted_str_ids.clone(),
        );
        let _ = app_handle.emit("sayl://tasks-changed", &payload);

        Ok(deleted_str_ids)
    }

    #[tauri::command]
    pub async fn delete_task(
        app_handle: tauri::AppHandle,
        state: State<'_, AppState>,
        id: String,
        delete_files: Option<bool>,
    ) -> Result<(), String> {
        delete_tasks(app_handle, state, vec![id], delete_files).await?;
        Ok(())
    }

    #[tauri::command]
    pub async fn list_tasks(state: State<'_, AppState>) -> Result<Vec<TaskView>, String> {
        let engine = state.engine.clone();
        let live_stats = Arc::clone(&state.live_stats);
        let parts_cache = Arc::clone(&state.parts_cache);
        let max_conn = state
            .settings
            .read()
            .map(|s| s.max_connections_per_task)
            .unwrap_or(4);

        tauri::async_runtime::spawn_blocking(move || {
            let records = engine
                .list_tasks()
                .map_err(|e| format!("Failed to list tasks: {e}"))?;

            let stats_guard = live_stats.lock().ok();
            let views = records
                .into_iter()
                .map(|rec| {
                    let stats = stats_guard.as_ref().and_then(|m| m.get(&rec.id).copied());
                    let parts = get_cached_parts(&engine, &parts_cache, rec.id);
                    let connections = estimate_connections(rec.state, parts, max_conn);
                    TaskView::from_record_full(
                        &rec,
                        stats.map(|s| s.speed_bps),
                        stats.and_then(|s| s.eta_secs),
                        parts,
                        connections,
                    )
                })
                .collect();

            Ok(views)
        })
        .await
        .map_err(|e| format!("Join error in list_tasks: {e}"))?
    }

    #[tauri::command]
    pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
        state
            .settings
            .read()
            .map(|s| s.clone())
            .map_err(|e| format!("Failed to read settings: {e}"))
    }

    #[tauri::command]
    pub fn set_settings(
        state: State<'_, AppState>,
        settings: Settings,
    ) -> Result<Settings, String> {
        validate_settings(&settings)?;

        let prev_run_at_startup = state
            .settings
            .read()
            .map(|s| s.run_at_startup)
            .unwrap_or(false);
        if prev_run_at_startup != settings.run_at_startup {
            let _ = autostart::set_autostart(settings.run_at_startup);
        }

        {
            let mut w = state
                .settings
                .write()
                .map_err(|e| format!("Failed to lock settings for write: {e}"))?;
            *w = settings.clone();
        }

        // Persist settings to disk if possible
        if let Ok(json) = serde_json::to_string_pretty(&settings) {
            let _ = std::fs::write(&state.settings_path, json);
        }

        state
            .clipboard_monitor
            .store(settings.clipboard_monitor, Ordering::Relaxed);

        let active = state.active_count.load(Ordering::Relaxed);
        update_sleep_inhibition_with_count(&state.settings, &state.sleep_inhibitor, active);

        Ok(settings)
    }

    #[tauri::command]
    pub fn get_clipboard_url() -> Option<String> {
        clipboard::read_clipboard_text().and_then(|text| clipboard::extract_download_url(&text))
    }

    #[tauri::command]
    pub fn get_clipboard_urls() -> Vec<String> {
        clipboard::read_clipboard_text()
            .map(|text| clipboard::extract_download_urls(&text))
            .unwrap_or_default()
    }

    #[tauri::command]
    pub fn open_download_folder(state: State<'_, AppState>) -> Result<(), String> {
        let dir = state
            .settings
            .read()
            .map(|s| PathBuf::from(&s.download_dir))
            .unwrap_or_else(|_| state.engine.download_dir().to_path_buf());

        let _ = std::fs::create_dir_all(&dir);

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let mut cmd = std::process::Command::new("explorer");
            cmd.arg(&dir);
            cmd.creation_flags(0x08000000);
            cmd.spawn()
                .map_err(|e| format!("Failed to open directory in explorer: {e}"))?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = dir;
        }

        Ok(())
    }

    #[tauri::command]
    pub fn reveal_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
        let task_id = id
            .parse::<TaskId>()
            .map_err(|e| format!("Invalid task ID: {e}"))?;

        let record = state
            .engine
            .get_task(task_id)
            .map_err(|e| format!("Failed to get task: {e}"))?
            .ok_or_else(|| "Task not found".to_string())?;

        let file_path = record.save_path.join(&record.filename);

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            if file_path.exists() {
                let mut cmd = std::process::Command::new("explorer");
                cmd.arg(format!("/select,{}", file_path.display()));
                cmd.creation_flags(0x08000000);
                cmd.spawn()
                    .map_err(|e| format!("Failed to reveal file: {e}"))?;
            } else {
                let mut cmd = std::process::Command::new("explorer");
                cmd.arg(&record.save_path);
                cmd.creation_flags(0x08000000);
                cmd.spawn()
                    .map_err(|e| format!("Failed to open folder: {e}"))?;
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = file_path;
        }

        Ok(())
    }

    #[tauri::command]
    pub fn open_task_file(state: State<'_, AppState>, id: String) -> Result<(), String> {
        let task_id = id
            .parse::<TaskId>()
            .map_err(|e| format!("Invalid task ID: {e}"))?;

        let record = state
            .engine
            .get_task(task_id)
            .map_err(|e| format!("Failed to get task: {e}"))?
            .ok_or_else(|| "Task not found".to_string())?;

        let file_path = record.save_path.join(&record.filename);
        if !file_path.exists() {
            return Err(format!("File does not exist: {}", file_path.display()));
        }

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let mut cmd = std::process::Command::new("explorer");
            cmd.arg(&file_path);
            cmd.creation_flags(0x08000000);
            cmd.spawn()
                .map_err(|e| format!("Failed to open file: {e}"))?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = file_path;
        }

        Ok(())
    }

    #[tauri::command]
    pub fn arm_shutdown(
        state: State<'_, AppState>,
        seconds: Option<u64>,
    ) -> Result<ShutdownStatus, String> {
        Ok(state.shutdown_controller.arm(seconds))
    }

    #[tauri::command]
    pub fn cancel_shutdown(state: State<'_, AppState>) -> Result<ShutdownStatus, String> {
        Ok(state.shutdown_controller.cancel())
    }

    #[tauri::command]
    pub fn get_shutdown_status(state: State<'_, AppState>) -> Result<ShutdownStatus, String> {
        Ok(state.shutdown_controller.status())
    }

    #[tauri::command]
    pub fn report_frontend_perf(payload: perf::FrontendPerf) -> Result<(), String> {
        perf::log_frontend_perf(&payload);
        Ok(())
    }

    #[tauri::command]
    pub fn native_host_status() -> NativeHostStatus {
        native_host::cmd_native_host_status()
    }

    #[tauri::command]
    pub async fn browser_status() -> Vec<browsers::BrowserStatus> {
        native_host::cmd_browser_status().await
    }

    #[tauri::command]
    pub fn prepare_extension() -> Result<String, String> {
        native_host::cmd_prepare_extension()
    }

    #[tauri::command]
    pub fn register_native_host(extension_id: Option<String>) -> Result<(), String> {
        native_host::cmd_register_native_host(extension_id)
    }

    #[tauri::command]
    pub fn unregister_native_host() -> Result<(), String> {
        native_host::cmd_unregister_native_host()
    }

    #[tauri::command]
    pub async fn test_native_host() -> Result<NativeHostPing, String> {
        native_host::cmd_test_native_host().await
    }

    #[tauri::command]
    pub fn extension_seen_age_ms() -> Option<u64> {
        #[derive(serde::Deserialize)]
        struct ExtensionSeenRecord {
            unix_ms: u64,
        }

        let path = default_data_dir().join("extension-seen.json");
        let contents = std::fs::read_to_string(&path).ok()?;
        let record: ExtensionSeenRecord = serde_json::from_str(&contents).ok()?;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis() as u64;
        Some(now_ms.saturating_sub(record.unix_ms))
    }

    #[tauri::command]
    pub fn open_extension_folder() -> Result<(), String> {
        native_host::cmd_open_extension_folder()
    }

    #[tauri::command]
    pub fn open_browser_extensions(browser_id: String) -> Result<(), String> {
        native_host::cmd_open_browser_extensions(browser_id)
    }

    #[tauri::command]
    pub async fn open_download_window(
        app_handle: tauri::AppHandle,
        state: State<'_, AppState>,
        id: String,
    ) -> Result<(), String> {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            return Err("Task ID cannot be empty".into());
        }
        let label = format!("download-{}", trimmed);
        if let Some(win) = app_handle.get_webview_window(&label) {
            let _ = win.show();
            let _ = win.set_focus();
            tracing::info!("download window shown: {label}");
            return Ok(());
        }

        let is_dark = {
            let theme_str = state
                .settings
                .read()
                .map(|s| s.theme.clone())
                .unwrap_or_else(|_| "system".into());
            if theme_str == "dark" {
                true
            } else if theme_str == "light" {
                false
            } else if let Some(main_win) = app_handle.get_webview_window("main") {
                main_win
                    .theme()
                    .map(|t| matches!(t, tauri::Theme::Dark))
                    .unwrap_or(false)
            } else {
                false
            }
        };

        let language = state
            .settings
            .read()
            .map(|s| s.language.clone())
            .unwrap_or_else(|_| "ar".into());
        let title = if language == "en" {
            "Download"
        } else {
            "تنزيل"
        };

        let bg_color = if is_dark {
            tauri::window::Color(31, 41, 55, 255) // #1f2937
        } else {
            tauri::window::Color(255, 255, 255, 255) // #ffffff
        };

        let url = format!("index.html?task={}", trimmed);
        let window = tauri::WebviewWindowBuilder::new(
            &app_handle,
            &label,
            tauri::WebviewUrl::App(url.into()),
        )
        .title(title)
        .visible(false)
        .inner_size(600.0, 440.0)
        .resizable(false)
        .decorations(true)
        .background_color(bg_color)
        .build()
        .map_err(|e| format!("Failed to create download window: {e}"))?;

        // Show and focus the window immediately after creation
        let _ = window.show();
        let _ = window.set_focus();
        tracing::info!("download window created and shown: {label}");
        Ok(())
    }

    #[tauri::command]
    pub fn close_download_window(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
        let label = format!("download-{}", id);
        if let Some(win) = app_handle.get_webview_window(&label) {
            let _ = win.close();
        }
        let _ = app_handle.emit("sayl://window-closed", &id);
        Ok(())
    }

    #[tauri::command]
    pub async fn open_complete_window(
        app_handle: tauri::AppHandle,
        state: State<'_, AppState>,
        id: String,
    ) -> Result<(), String> {
        let settings = state
            .settings
            .read()
            .map_err(|_| "Settings lock poisoned".to_string())?
            .clone();
        completion::open_complete_window(&app_handle, &id, &settings)
    }

    #[tauri::command]
    pub fn close_complete_window(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
        completion::close_complete_window(&app_handle, &id);
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Application Setup & Runner
// ----------------------------------------------------------------------------

pub fn run() {
    let data_dir = default_data_dir();
    let _ = std::fs::create_dir_all(&data_dir);
    perf::init(&data_dir);
    perf::log_event("process_start");

    let settings_path = data_dir.join("settings.json");
    let initial_settings = if settings_path.exists() {
        std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
            .unwrap_or_default()
    } else {
        Settings::default()
    };

    let download_dir = PathBuf::from(&initial_settings.download_dir);
    let store_dir = data_dir.join("store");

    let engine_config = EngineConfig {
        max_connections: initial_settings.max_connections_per_task,
        max_concurrent_tasks: initial_settings.max_concurrent_tasks,
        chunk_size: EngineConfig::default().chunk_size,
        ..Default::default()
    };

    let engine = match Engine::new(&download_dir, &store_dir, engine_config) {
        Ok(e) => {
            perf::log_event("engine_init");
            e
        }
        Err(e) => {
            eprintln!("Failed to initialize download engine: {e}");
            panic!("Failed to initialize download engine: {e}");
        }
    };

    let sleep_inhibitor = Arc::new(SleepInhibitor::new());
    let app_handle_slot: Arc<OnceLock<tauri::AppHandle>> = Arc::new(OnceLock::new());

    let shutdown_runner: Arc<dyn ShutdownRunner> = Arc::new(shutdown::SystemShutdownRunner);
    let engine_for_shutdown = engine.clone();
    let shutdown_states: Arc<dyn Fn() -> Vec<DownloadState> + Send + Sync> = Arc::new(move || {
        engine_for_shutdown
            .list_tasks()
            .map(|tasks| tasks.into_iter().map(|t| t.state).collect())
            .unwrap_or_default()
    });
    let app_handle_for_notify = Arc::clone(&app_handle_slot);
    let shutdown_notify: Arc<dyn Fn(ShutdownStatus) + Send + Sync> = Arc::new(move |status| {
        if let Some(app_handle) = app_handle_for_notify.get() {
            let _ = app_handle.emit("sayl://shutdown", &status);
        }
    });

    let shutdown_controller = Arc::new(ShutdownController::new(
        shutdown_runner,
        shutdown_states,
        shutdown_notify,
    ));

    let app_state = AppState::new(
        engine,
        initial_settings,
        settings_path,
        sleep_inhibitor,
        shutdown_controller,
    );

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::add_task,
            commands::add_task_queued,
            commands::start_task,
            commands::pause_task,
            commands::cancel_task,
            commands::set_task_speed_limit,
            commands::delete_tasks,
            commands::delete_task,
            commands::list_tasks,
            commands::get_settings,
            commands::set_settings,
            commands::get_clipboard_url,
            commands::get_clipboard_urls,
            commands::open_download_folder,
            commands::reveal_task,
            commands::open_task_file,
            commands::arm_shutdown,
            commands::cancel_shutdown,
            commands::get_shutdown_status,
            commands::report_frontend_perf,
            commands::native_host_status,
            commands::browser_status,
            commands::prepare_extension,
            commands::register_native_host,
            commands::unregister_native_host,
            commands::test_native_host,
            commands::extension_seen_age_ms,
            commands::open_extension_folder,
            commands::open_browser_extensions,
            commands::open_download_window,
            commands::close_download_window,
            commands::open_complete_window,
            commands::close_complete_window,
            commands::prepare_capture_task,
            commands::extension_diagnostics
        ])
        .on_page_load(move |_webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                static ONCE: OnceLock<()> = OnceLock::new();
                if ONCE.set(()).is_ok() {
                    perf::log_event("window_shown");
                }
            }
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let label = window.label();
                if let Some(task_id) = label.strip_prefix("download-") {
                    let _ = window.app_handle().emit("sayl://window-closed", task_id);
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != "main" {
                    return;
                }
                let app = window.app_handle();
                if let Some(state) = app.try_state::<AppState>() {
                    let (close_action, close_notice_shown, language) = {
                        let s = state.settings.read().unwrap();
                        (
                            s.close_action.clone(),
                            s.close_notice_shown,
                            s.language.clone(),
                        )
                    };

                    if close_action == "background" {
                        api.prevent_close();
                        let _ = window.hide();

                        if !close_notice_shown {
                            let (title, body) = if language == "en" {
                                (
                                    "SAYL is running in the background",
                                    "SAYL will continue downloading your files in the system tray.",
                                )
                            } else {
                                (
                                    "سيل يعمل في الخلفية",
                                    "سيستمر سيل في تنزيل ملفاتك في منطقة الإشعار.",
                                )
                            };

                            use tauri_plugin_notification::NotificationExt;
                            let _ = app.notification().builder().title(title).body(body).show();

                            if let Ok(mut w) = state.settings.write() {
                                w.close_notice_shown = true;
                                if let Ok(json) = serde_json::to_string_pretty(&*w) {
                                    let _ = std::fs::write(&state.settings_path, json);
                                }
                            }
                        }
                    }
                }
            }
        })
        .setup(move |app| {
            perf::log_event("setup_done");
            let _ = app_handle_slot.set(app.handle().clone());

            // Setup system tray icon and menu
            let tray_handle = tray::setup_tray(app.handle()).ok();

            // Auto-repair & re-register native host for all browsers silently on start.
            // Executed in the background pool so a slow registry write never delays app startup.
            tauri::async_runtime::spawn_blocking(|| {
                let mut reg = sayl_bridge::install::WindowsRegistry;
                match native_host::register_native_host_with_ops(&mut reg, None) {
                    Ok(()) => {
                        tracing::info!("native messaging host auto-registered and repaired");
                    }
                    Err(e) => {
                        tracing::warn!("auto-registration of native messaging host failed: {e}");
                    }
                }
            });

            // Handle --hidden startup argument
            if autostart::is_hidden_arg(std::env::args()) {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            let state = app.state::<AppState>();
            let engine = state.engine.clone();
            let live_stats = Arc::clone(&state.live_stats);
            let parts_cache = Arc::clone(&state.parts_cache);
            let active_count_atomic = Arc::clone(&state.active_count);
            let app_handle = app.handle().clone();
            let sleep_inhibitor = Arc::clone(&state.sleep_inhibitor);

            // Start clipboard poller thread
            let app_handle_for_clip = app.handle().clone();
            let clip_enabled = Arc::clone(&state.clipboard_monitor);
            std::thread::Builder::new()
                .name("clipboard-poller".to_string())
                .spawn(move || {
                    let mut monitor_state = clipboard::ClipboardMonitorState::new();
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        let enabled = clip_enabled.load(Ordering::Relaxed);
                        if !enabled {
                            continue;
                        }
                        // Only read the clipboard while enabled is true
                        let current_text = clipboard::read_clipboard_text();
                        if let Some(url) = monitor_state.tick(enabled, current_text) {
                            // Surface the main window so AddTaskDialog is visible (Gap 1)
                            if let Some(window) = app_handle_for_clip.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }

                            #[derive(Serialize, Clone)]
                            struct ClipboardUrlPayload {
                                url: String,
                            }
                            let _ = app_handle_for_clip.emit(
                                "sayl://clipboard-url",
                                ClipboardUrlPayload { url },
                            );
                        }
                    }
                })
                .expect("failed to spawn clipboard poller thread");

            // Auto-register native host for all browsers on every start (silent repair)
            let app_handle_for_host = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Small delay to let the app fully start
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let _ = app_handle_for_host
                    .try_state::<AppState>()
                    .map(|state| {
                        let settings = state.settings.read().ok();
                        if settings.is_some() {
                            // Only re-register if the user hasn't explicitly disabled it
                            // This is silent - no user prompts
                            let _ = crate::commands::register_native_host(None);
                        }
                    });
            });

            // Start loopback IPC capture server
            let capture_engine = engine.clone();
            let capture_data_dir = default_data_dir();
            let capture_app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match capture_server::start_capture_server(
                    capture_engine,
                    capture_data_dir,
                    None,
                    Some(capture_app_handle),
                )
                .await
                {
                    Ok(handle) => {
                        Box::leak(Box::new(handle));
                    }
                    Err(e) => {
                        eprintln!("Failed to start capture loopback server: {e}");
                    }
                }
            });

            tauri::async_runtime::spawn(async move {
                // Recover non-terminal tasks from durable storage
                let recovered = engine.recover().await.unwrap_or_default();
                println!(
                    "Engine recovery completed: {} tasks recovered",
                    recovered.len()
                );

                // Build initial parts cache and initial task views in background
                let initial_views = {
                    let eng = engine.clone();
                    let pc = Arc::clone(&parts_cache);
                    let max_c = app_handle
                        .try_state::<AppState>()
                        .and_then(|s| {
                            s.settings
                                .read()
                                .ok()
                                .map(|st| st.max_connections_per_task)
                        })
                        .unwrap_or(4);
                    tauri::async_runtime::spawn_blocking(move || {
                        let records = eng.list_tasks().unwrap_or_default();
                        let mut active = 0;
                        let views: Vec<TaskView> = records
                            .into_iter()
                            .map(|rec| {
                                if is_active_state(rec.state) {
                                    active += 1;
                                }
                                let parts = get_cached_parts(&eng, &pc, rec.id);
                                let connections = estimate_connections(rec.state, parts, max_c);
                                TaskView::from_record_full(&rec, None, None, parts, connections)
                            })
                            .collect();
                        (views, active)
                    })
                    .await
                    .unwrap_or_default()
                };

                let (initial_task_views, initial_active) = initial_views;
                active_count_atomic.store(initial_active, Ordering::SeqCst);

                if let Some(app_state) = app_handle.try_state::<AppState>() {
                    update_sleep_inhibition_with_count(
                        &app_state.settings,
                        &sleep_inhibitor,
                        initial_active,
                    );
                }

                // Initial full emission on startup/recovery completion
                let tasks_revision = app_handle
                    .try_state::<AppState>()
                    .map(|s| Arc::clone(&s.tasks_revision));
                let initial_revision = match tasks_revision.as_ref() {
                    Some(counter) => counter.load(Ordering::Relaxed),
                    None => INITIAL_TASKS_REVISION,
                };
                let initial_batch = TasksChangedPayload {
                    revision: initial_revision,
                    full: true,
                    upsert: initial_task_views,
                    removed: Vec::new(),
                };
                let _ = app_handle.emit("sayl://tasks-changed", &initial_batch);

                // Event coalescing pump: 250ms ticker
                let mut rx = match engine.take_event_receiver() {
                    Some(r) => r,
                    None => return,
                };

                let mut ticker = tokio::time::interval(std::time::Duration::from_millis(250));
                ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                let mut pending_upsert_ids: std::collections::HashSet<TaskId> =
                    std::collections::HashSet::new();
                let mut pending_removed_ids: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                let mut ticker_counter: u32 = 0;
                let mut fallback_revision: u64 = initial_revision;

                loop {
                    tokio::select! {
                        event_opt = rx.recv() => {
                            let event = match event_opt {
                                Some(e) => e,
                                None => break,
                            };
                            let task_id = event.task_id;

                            match &event.payload {
                                sayl_core::event::EventPayload::Progress {
                                    downloaded_bytes,
                                    total_bytes,
                                    speed_bps,
                                } => {
                                    let eta = if *speed_bps > 0 {
                                        total_bytes.and_then(|tot| {
                                            if tot > *downloaded_bytes {
                                                Some((tot - *downloaded_bytes) / *speed_bps)
                                            } else {
                                                None
                                            }
                                        })
                                    } else {
                                        None
                                    };

                                    if let Ok(mut stats) = live_stats.lock() {
                                        stats.insert(
                                            task_id,
                                            LiveTaskStats {
                                                speed_bps: *speed_bps,
                                                eta_secs: eta,
                                            },
                                        );
                                    }

                                    let progress = ProgressPayload {
                                        id: task_id.to_string(),
                                        written: *downloaded_bytes,
                                        total: *total_bytes,
                                        speed_bps: *speed_bps,
                                    };
                                    let _ = app_handle.emit("sayl://progress", &progress);
                                }
                                sayl_core::event::EventPayload::StateChanged { from, to, .. } => {
                                    if *to == DownloadState::Completed && *from != DownloadState::Completed {
                                        completion::handle_download_completed(
                                            app_handle.clone(),
                                            engine.clone(),
                                            task_id,
                                        );
                                    }
                                    if to.is_terminal() || *to == DownloadState::Paused {
                                        if let Ok(mut stats) = live_stats.lock() {
                                            stats.remove(&task_id);
                                        }
                                    }
                                    // Update active count & sleep inhibition
                                    if let Some(app_state) = app_handle.try_state::<AppState>() {
                                        let current_active = if let Ok(tasks) = engine.list_tasks() {
                                            tasks.into_iter().filter(|t| is_active_state(t.state)).count()
                                        } else {
                                            0
                                        };
                                        active_count_atomic.store(current_active, Ordering::Relaxed);
                                        update_sleep_inhibition_with_count(
                                            &app_state.settings,
                                            &sleep_inhibitor,
                                            current_active,
                                        );
                                    }
                                    pending_upsert_ids.insert(task_id);
                                }
                                sayl_core::event::EventPayload::Split { .. }
                                | sayl_core::event::EventPayload::Checkpoint { .. } => {
                                    // Invalidate parts cache entry so it recalculates on next access
                                    if let Ok(mut cache) = parts_cache.write() {
                                        cache.remove(&task_id);
                                    }
                                    pending_upsert_ids.insert(task_id);
                                }
                                _ => {
                                    pending_upsert_ids.insert(task_id);
                                }
                            }
                        }
                        _ = ticker.tick() => {
                            ticker_counter = ticker_counter.wrapping_add(1);

                            // Update tray tooltip ~1/s (every 4 ticks of 250ms)
                            if ticker_counter.is_multiple_of(4) {
                                if let Some(ref tray) = tray_handle {
                                    let total_speed: u64 = live_stats
                                        .lock()
                                        .map(|s| s.values().map(|v| v.speed_bps).sum())
                                        .unwrap_or(0);
                                    let active = active_count_atomic.load(Ordering::Relaxed);
                                    let lang = app_handle
                                        .try_state::<AppState>()
                                        .and_then(|s| s.settings.read().ok().map(|st| st.language.clone()))
                                        .unwrap_or_else(|| "ar".to_string());
                                    let tooltip = tray::format_tooltip(&lang, total_speed, active);
                                    let _ = tray.set_tooltip(Some(tooltip));
                                }
                            }

                            // Flush pending changes if any
                            if !pending_upsert_ids.is_empty() || !pending_removed_ids.is_empty() {
                                let upsert_ids: Vec<TaskId> = pending_upsert_ids.drain().collect();
                                let removed_ids: Vec<String> = pending_removed_ids.drain().collect();

                                // Shared counter, so a concurrent delete command cannot
                                // hand out the same revision as this batch.
                                let revision = match tasks_revision.as_ref() {
                                    Some(counter) => next_tasks_revision(counter),
                                    None => {
                                        fallback_revision += 1;
                                        fallback_revision
                                    }
                                };
                                let eng = engine.clone();
                                let stats_arc = Arc::clone(&live_stats);
                                let pc = Arc::clone(&parts_cache);
                                let max_c = app_handle
                                    .try_state::<AppState>()
                                    .and_then(|s| s.settings.read().ok().map(|st| st.max_connections_per_task))
                                    .unwrap_or(4);

                                let batch = tauri::async_runtime::spawn_blocking(move || {
                                    let stats_guard = stats_arc.lock().ok();
                                    build_tasks_changed_batch(
                                        revision,
                                        false,
                                        &upsert_ids,
                                        &removed_ids,
                                        |id| {
                                            eng.get_task(id).ok().flatten().map(|rec| {
                                                let stats = stats_guard.as_ref().and_then(|m| m.get(&id).copied());
                                                let parts = get_cached_parts(&eng, &pc, id);
                                                let connections = estimate_connections(rec.state, parts, max_c);
                                                TaskView::from_record_full(
                                                    &rec,
                                                    stats.map(|s| s.speed_bps),
                                                    stats.and_then(|s| s.eta_secs),
                                                    parts,
                                                    connections,
                                                )
                                            })
                                        },
                                        Vec::new,
                                    )
                                })
                                .await
                                .unwrap_or_else(|_| TasksChangedPayload {
                                    revision,
                                    full: false,
                                    upsert: Vec::new(),
                                    removed: Vec::new(),
                                });

                                let _ = app_handle.emit("sayl://tasks-changed", &batch);
                            }
                        }
                    }
                }
            });

            Ok(())
        });

    let mut context = tauri::generate_context!();
    context.config_mut().app.app_directories_override =
        Some(tauri::utils::config::AppDirectoriesOverride::Root(data_dir));

    builder
        .build(context)
        .expect("error while building SAYL desktop application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                perf::log_event("exit");
                let ipc_file = default_data_dir().join("capture-ipc.json");
                let _ = std::fs::remove_file(ipc_file);
            }
        });
}

// ----------------------------------------------------------------------------
// Unit Tests (Pure helpers testable without webview)
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sayl_core::state::DownloadState;

    #[test]
    fn test_settings_validation_valid() {
        let valid = Settings {
            download_dir: "C:\\Users\\User\\Downloads".to_string(),
            max_concurrent_tasks: 4,
            max_connections_per_task: 8,
            theme: "dark".to_string(),
            language: "ar".to_string(),
            ..Settings::default()
        };
        assert!(validate_settings(&valid).is_ok());
    }

    #[test]
    fn test_settings_validation_invalid_bounds() {
        let empty_dir = Settings {
            download_dir: "   ".to_string(),
            ..Settings::default()
        };
        assert!(validate_settings(&empty_dir).is_err());

        let zero_tasks = Settings {
            max_concurrent_tasks: 0,
            ..Settings::default()
        };
        assert!(validate_settings(&zero_tasks).is_err());

        let excessive_tasks = Settings {
            max_concurrent_tasks: 50,
            ..Settings::default()
        };
        assert!(validate_settings(&excessive_tasks).is_err());

        let invalid_theme = Settings {
            theme: "neon-blue".to_string(),
            ..Settings::default()
        };
        assert!(validate_settings(&invalid_theme).is_err());

        let invalid_lang = Settings {
            language: "fr".to_string(),
            ..Settings::default()
        };
        assert!(validate_settings(&invalid_lang).is_err());
    }

    #[test]
    fn test_settings_validation_view_mode() {
        let cards = Settings {
            view_mode: "cards".to_string(),
            ..Settings::default()
        };
        assert!(validate_settings(&cards).is_ok());

        let table = Settings {
            view_mode: "table".to_string(),
            ..Settings::default()
        };
        assert!(validate_settings(&table).is_ok());

        let grid = Settings {
            view_mode: "grid".to_string(),
            ..Settings::default()
        };
        assert!(validate_settings(&grid).is_err());
        assert_eq!(
            validate_settings(&grid).unwrap_err(),
            "View mode must be cards or table"
        );
    }

    #[test]
    fn test_settings_validation_close_action() {
        let background = Settings {
            close_action: "background".to_string(),
            ..Settings::default()
        };
        assert!(validate_settings(&background).is_ok());

        let quit = Settings {
            close_action: "quit".to_string(),
            ..Settings::default()
        };
        assert!(validate_settings(&quit).is_ok());

        let minimize = Settings {
            close_action: "minimize".to_string(),
            ..Settings::default()
        };
        assert!(validate_settings(&minimize).is_err());
        assert_eq!(
            validate_settings(&minimize).unwrap_err(),
            "Close action must be background or quit"
        );
    }

    #[test]
    fn test_settings_backward_compat_and_serialized_keys() {
        let old_json = r#"{
            "download_dir": "C:\\Downloads",
            "max_concurrent_tasks": 3,
            "max_connections_per_task": 4,
            "theme": "dark",
            "language": "en"
        }"#;

        let settings: Settings = serde_json::from_str(old_json).expect("parses old json");
        assert_eq!(settings.download_dir, "C:\\Downloads");
        assert_eq!(settings.max_concurrent_tasks, 3);
        assert_eq!(settings.max_connections_per_task, 4);
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.language, "en");
        assert_eq!(settings.view_mode, "table");
        assert!(!settings.sidebar_collapsed);
        assert!(settings.prevent_sleep);
        assert!(settings.show_download_window);
        assert!(settings.show_download_complete_window);
        assert!(!settings.close_download_window_on_finish);
        assert_eq!(settings.close_action, "background");
        assert!(!settings.run_at_startup);
        assert!(!settings.close_notice_shown);
        assert!(settings.clipboard_monitor);

        let val: serde_json::Value = serde_json::to_value(&settings).expect("serializes");
        let obj = val.as_object().expect("object");
        assert_eq!(obj.len(), 15);
        let expected_keys = [
            "download_dir",
            "max_concurrent_tasks",
            "max_connections_per_task",
            "theme",
            "language",
            "view_mode",
            "sidebar_collapsed",
            "prevent_sleep",
            "show_download_window",
            "show_download_complete_window",
            "close_download_window_on_finish",
            "close_action",
            "run_at_startup",
            "close_notice_shown",
            "clipboard_monitor",
        ];
        for key in expected_keys {
            assert!(obj.contains_key(key), "missing key: {key}");
        }
    }

    #[test]
    fn test_settings_clipboard_monitor_default_and_backward_compat() {
        let default_settings = Settings::default();
        assert!(default_settings.clipboard_monitor);

        let json_without = r#"{"download_dir":"C:\\Downloads"}"#;
        let s1: Settings = serde_json::from_str(json_without).expect("deserializes without field");
        assert!(s1.clipboard_monitor);

        let json_false = r#"{"download_dir":"C:\\Downloads","clipboard_monitor":false}"#;
        let s2: Settings = serde_json::from_str(json_false).expect("deserializes with false");
        assert!(!s2.clipboard_monitor);
    }

    #[test]
    fn test_build_tasks_changed_batch_coalescing() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();

        // 1. Full batch
        let full_batch = build_tasks_changed_batch(
            1,
            true,
            &[id1, id1],
            &["rem1".to_string()],
            |_| None,
            || {
                vec![TaskView {
                    id: id1.to_string(),
                    url: "https://example.com/1".to_string(),
                    filename: "1.zip".to_string(),
                    state: "Downloading".to_string(),
                    total_size: None,
                    written: 0,
                    speed_bps: None,
                    eta_secs: None,
                    error: None,
                    mime: None,
                    parts: None,
                    error_kind: None,
                    connections: None,
                    created_at: None,
                    completed_at: None,
                    folder: String::new(),
                    speed_limit_kbps: None,
                }]
            },
        );
        assert_eq!(full_batch.revision, 1);
        assert!(full_batch.full);
        assert_eq!(full_batch.upsert.len(), 1);
        assert_eq!(full_batch.removed.len(), 0);

        // 2. Incremental batch with duplicate task ids and duplicate removed ids
        let inc_batch = build_tasks_changed_batch(
            2,
            false,
            &[id1, id2, id1],
            &["rem1".to_string(), "rem1".to_string(), "rem2".to_string()],
            |id| {
                Some(TaskView {
                    id: id.to_string(),
                    url: "https://example.com/file".to_string(),
                    filename: "file.bin".to_string(),
                    state: "Completed".to_string(),
                    total_size: None,
                    written: 100,
                    speed_bps: None,
                    eta_secs: None,
                    error: None,
                    mime: None,
                    parts: None,
                    error_kind: None,
                    connections: None,
                    created_at: None,
                    completed_at: None,
                    folder: String::new(),
                    speed_limit_kbps: None,
                })
            },
            Vec::new,
        );
        assert_eq!(inc_batch.revision, 2);
        assert!(!inc_batch.full);
        assert_eq!(inc_batch.upsert.len(), 2);
        assert_eq!(inc_batch.removed.len(), 2);
        assert_eq!(
            inc_batch.removed,
            vec!["rem1".to_string(), "rem2".to_string()]
        );
    }

    #[test]
    fn test_delete_payload_uses_real_monotonic_revision() {
        let counter = AtomicU64::new(INITIAL_TASKS_REVISION);

        // A deletion takes the next revision from the shared counter — never 0,
        // and never the revision already used by the initial full batch.
        let first = next_tasks_revision(&counter);
        assert_eq!(first, INITIAL_TASKS_REVISION + 1);

        let payload = build_removed_tasks_changed_payload(
            first,
            vec!["01900000-0000-7000-8000-000000000042".to_string()],
        );
        assert_ne!(payload.revision, 0);
        assert_eq!(payload.revision, first);
        assert!(!payload.full);
        assert!(payload.upsert.is_empty());
        assert_eq!(payload.removed.len(), 1);

        // The next change (delete or coalescing pump) gets a strictly larger revision.
        let second = next_tasks_revision(&counter);
        assert!(second > first);
        assert_eq!(second, first + 1);
    }

    #[test]
    fn test_delete_payload_serde_roundtrip_keeps_revision() {
        let payload = build_removed_tasks_changed_payload(7, vec!["task-a".to_string()]);
        let json = serde_json::to_string(&payload).expect("serializes");
        let decoded: TasksChangedPayload = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(decoded, payload);
        assert_eq!(decoded.revision, 7);
        assert_eq!(decoded.removed, vec!["task-a".to_string()]);
    }

    #[test]
    fn test_estimate_connections() {
        // Downloading with parts and cap
        assert_eq!(
            estimate_connections(DownloadState::Downloading, Some(8), 4),
            Some(4)
        );
        assert_eq!(
            estimate_connections(DownloadState::Downloading, Some(2), 4),
            Some(2)
        );
        assert_eq!(
            estimate_connections(DownloadState::Downloading, None, 4),
            Some(1)
        );

        // Probing
        assert_eq!(
            estimate_connections(DownloadState::Probing, None, 4),
            Some(1)
        );

        // Inactive states
        assert_eq!(
            estimate_connections(DownloadState::Queued, Some(4), 4),
            None
        );
        assert_eq!(
            estimate_connections(DownloadState::Paused, Some(4), 4),
            None
        );
        assert_eq!(
            estimate_connections(DownloadState::Completed, Some(4), 4),
            None
        );
        assert_eq!(
            estimate_connections(DownloadState::Cancelled, Some(4), 4),
            None
        );
        assert_eq!(
            estimate_connections(DownloadState::Failed, Some(4), 4),
            None
        );
    }

    #[test]
    fn test_task_view_rfc3339_and_completed_at() {
        let mut rec = TaskRecord::new(
            "https://example.com/test.iso",
            PathBuf::from("C:\\Downloads"),
            "test.iso",
        );
        rec.state = DownloadState::Completed;

        let view = TaskView::from_record_full(&rec, None, None, Some(4), None);
        assert!(view.created_at.is_some());
        assert!(view.completed_at.is_some());
        assert_eq!(view.completed_at, Some(rec.updated_at.to_rfc3339()));

        rec.state = DownloadState::Downloading;
        let running_view = TaskView::from_record_full(&rec, None, None, Some(4), Some(4));
        assert!(running_view.created_at.is_some());
        assert_eq!(running_view.completed_at, None);
        assert_eq!(running_view.connections, Some(4));
    }

    #[test]
    fn test_download_dir_validation_strict() {
        // Valid paths: absolute directories with at least one normal component
        assert!(validate_download_dir("C:\\Users\\User\\Downloads").is_ok());
        assert!(validate_download_dir("C:\\Downloads").is_ok());
        assert!(validate_download_dir("D:\\Media\\Files").is_ok());

        // Relative paths rejected
        assert!(validate_download_dir("downloads").is_err());
        assert!(validate_download_dir(".\\downloads").is_err());
        assert!(validate_download_dir("relative/path").is_err());
        assert!(validate_download_dir("").is_err());
        assert!(validate_download_dir("   ").is_err());

        // ParentDir '..' components rejected
        assert!(validate_download_dir("C:\\Downloads\\..\\evil").is_err());
        assert!(validate_download_dir("C:\\..\\Windows").is_err());
        assert!(validate_download_dir("C:/Downloads/../evil").is_err());

        // Bare drive root rejected
        assert!(validate_download_dir("C:\\").is_err());
        assert!(validate_download_dir("C:/").is_err());
        assert!(validate_download_dir("D:\\").is_err());
        assert!(validate_download_dir("C:").is_err());

        // Empty components (repeated separators) rejected
        assert!(validate_download_dir("C:\\Downloads\\\\sub").is_err());
        assert!(validate_download_dir("C:/Downloads//sub").is_err());
    }

    #[test]
    fn test_task_view_mapping_from_record() {
        let record = TaskRecord::new(
            "https://example.com/archive.zip",
            PathBuf::from("C:\\Downloads"),
            "archive.zip",
        );

        let view = TaskView::from_record(&record, Some(1048576), Some(120), None);
        assert_eq!(view.id, record.id.to_string());
        assert_eq!(view.url, "https://example.com/archive.zip");
        assert_eq!(view.filename, "archive.zip");
        assert_eq!(view.state, "Queued");
        assert_eq!(view.written, 0);
        // Because state is Queued (not Downloading/Probing), transient speed/eta should be None
        assert_eq!(view.speed_bps, None);
        assert_eq!(view.eta_secs, None);
        assert_eq!(view.error, None);
        assert_eq!(view.mime, None);
        assert_eq!(view.parts, None);
    }

    #[test]
    fn test_task_view_downloading_stats_preservation() {
        let mut record = TaskRecord::new(
            "https://example.com/video.mp4",
            PathBuf::from("C:\\Downloads"),
            "video.mp4",
        );
        record.state = DownloadState::Downloading;
        record.written = 50_000_000;
        record.total_size = Some(100_000_000);

        let view = TaskView::from_record(&record, Some(2_000_000), Some(25), None);
        assert_eq!(view.state, "Downloading");
        assert_eq!(view.speed_bps, Some(2_000_000));
        assert_eq!(view.eta_secs, Some(25));
        assert_eq!(view.written, 50_000_000);
        assert_eq!(view.total_size, Some(100_000_000));
        assert_eq!(view.mime, None);
        assert_eq!(view.parts, None);
    }

    #[test]
    fn test_task_view_mime_and_parts_mapping() {
        let mut record = TaskRecord::new(
            "https://example.com/archive.tar.gz",
            PathBuf::from("C:\\Downloads"),
            "archive.tar.gz",
        );
        record.content_type = Some("application/gzip".to_string());

        let view = TaskView::from_record(&record, None, None, Some(4));
        assert_eq!(view.mime, Some("application/gzip".to_string()));
        assert_eq!(view.parts, Some(4));

        record.content_type = None;
        let view_empty = TaskView::from_record(&record, None, None, None);
        assert_eq!(view_empty.mime, None);
        assert_eq!(view_empty.parts, None);
    }

    #[test]
    fn test_progress_payload_serde() {
        let p = ProgressPayload {
            id: "01900000-0000-7000-8000-000000000001".to_string(),
            written: 1024,
            total: Some(2048),
            speed_bps: 512,
        };

        let json = serde_json::to_string(&p).expect("serialization succeeds");
        let decoded: ProgressPayload =
            serde_json::from_str(&json).expect("deserialization succeeds");
        assert_eq!(p, decoded);
    }

    #[test]
    fn test_default_data_dir_override() {
        let custom_dir = std::env::temp_dir().join("sayl_test_data_dir");
        std::env::set_var("SAYL_DATA_DIR", &custom_dir);
        let resolved = default_data_dir();
        std::env::remove_var("SAYL_DATA_DIR");
        assert_eq!(resolved, custom_dir);
    }

    #[test]
    fn test_default_download_dir_override() {
        let custom_dir = std::env::temp_dir().join("sayl_test_dl_dir");
        std::env::set_var("SAYL_DOWNLOAD_DIR", &custom_dir);
        let resolved = default_download_dir();
        std::env::remove_var("SAYL_DOWNLOAD_DIR");
        assert_eq!(resolved, custom_dir);
    }
}
