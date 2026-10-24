//! SAYL Desktop Tauri backend.
//!
//! Orchestrates the `sayl-app` download engine, manages settings,
//! and communicates with the Svelte frontend via Tauri commands and events.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

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

/// User configurable settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    pub download_dir: String,
    pub max_concurrent_tasks: usize,
    pub max_connections_per_task: usize,
    pub theme: String,
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_dir: default_download_dir().to_string_lossy().to_string(),
            max_concurrent_tasks: 3,
            max_connections_per_task: 4,
            theme: "system".to_string(),
            language: "ar".to_string(),
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
    let check_str = if trimmed.starts_with(r"\\") {
        &trimmed[2..]
    } else {
        trimmed
    };
    if check_str.contains("//")
        || check_str.contains(r"\\")
        || check_str.contains(r"/\" )
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
    Ok(())
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
}

impl TaskView {
    /// Constructs a `TaskView` from a durable `TaskRecord` and optional transient stats.
    pub fn from_record(rec: &TaskRecord, speed_bps: Option<u64>, eta_secs: Option<u64>) -> Self {
        let is_running = rec.state == DownloadState::Downloading || rec.state == DownloadState::Probing;
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
        }
    }
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
    pub settings_path: PathBuf,
}

impl AppState {
    pub fn new(engine: Engine, settings: Settings, settings_path: PathBuf) -> Self {
        Self {
            engine,
            settings: RwLock::new(settings),
            live_stats: Arc::new(Mutex::new(HashMap::new())),
            settings_path,
        }
    }
}

/// Default download directory resolution.
pub fn default_download_dir() -> PathBuf {
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
    if let Some(app_data) = std::env::var_os("APPDATA") {
        PathBuf::from(app_data).join("SAYL")
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".sayl")
    }
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
    ) -> Result<TaskView, String> {
        let clean_url = url.trim();
        if clean_url.is_empty() {
            return Err("URL cannot be empty".to_string());
        }

        let task_id = state
            .engine
            .add(clean_url)
            .await
            .map_err(|e| format!("Failed to add task: {e}"))?;

        // Auto-start the newly added task
        state.engine.spawn_start(task_id);

        let record = state
            .engine
            .get_task(task_id)
            .map_err(|e| format!("Storage read failed: {e}"))?
            .ok_or_else(|| "Task record not found after insert".to_string())?;

        Ok(TaskView::from_record(&record, None, None))
    }

    #[tauri::command]
    pub async fn start_task(
        state: State<'_, AppState>,
        id: String,
    ) -> Result<(), String> {
        let task_id = id
            .parse::<TaskId>()
            .map_err(|e| format!("Invalid task ID: {e}"))?;

        state.engine.spawn_start(task_id);
        Ok(())
    }

    #[tauri::command]
    pub async fn pause_task(
        state: State<'_, AppState>,
        id: String,
    ) -> Result<(), String> {
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

        Ok(())
    }

    #[tauri::command]
    pub async fn cancel_task(
        state: State<'_, AppState>,
        id: String,
    ) -> Result<(), String> {
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

        Ok(())
    }

    #[tauri::command]
    pub async fn list_tasks(state: State<'_, AppState>) -> Result<Vec<TaskView>, String> {
        let records = state
            .engine
            .list_tasks()
            .map_err(|e| format!("Failed to list tasks: {e}"))?;

        let stats_guard = state.live_stats.lock().ok();
        let views = records
            .into_iter()
            .map(|rec| {
                let stats = stats_guard.as_ref().and_then(|m| m.get(&rec.id).copied());
                TaskView::from_record(
                    &rec,
                    stats.map(|s| s.speed_bps),
                    stats.and_then(|s| s.eta_secs),
                )
            })
            .collect();

        Ok(views)
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

        Ok(settings)
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
            std::process::Command::new("explorer")
                .arg(&dir)
                .spawn()
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
            if file_path.exists() {
                std::process::Command::new("explorer")
                    .arg(format!("/select,{}", file_path.display()))
                    .spawn()
                    .map_err(|e| format!("Failed to reveal file: {e}"))?;
            } else {
                std::process::Command::new("explorer")
                    .arg(&record.save_path)
                    .spawn()
                    .map_err(|e| format!("Failed to open folder: {e}"))?;
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = file_path;
        }

        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Application Setup & Runner
// ----------------------------------------------------------------------------

pub fn run() {
    let data_dir = default_data_dir();
    let _ = std::fs::create_dir_all(&data_dir);

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
    };

    let engine = match Engine::new(&download_dir, &store_dir, engine_config) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to initialize download engine: {e}");
            return;
        }
    };

    let app_state = AppState::new(engine, initial_settings, settings_path);

    let builder = tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::add_task,
            commands::start_task,
            commands::pause_task,
            commands::cancel_task,
            commands::list_tasks,
            commands::get_settings,
            commands::set_settings,
            commands::open_download_folder,
            commands::reveal_task
        ])
        .setup(|app| {
            let state = app.state::<AppState>();
            let engine = state.engine.clone();
            let live_stats = Arc::clone(&state.live_stats);
            let app_handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                // Recover non-terminal tasks from durable storage
                match engine.recover().await {
                    Ok(recovered) => {
                        println!("Engine recovery completed: {} tasks recovered", recovered.len());
                    }
                    Err(err) => {
                        eprintln!("Engine recovery error: {err}");
                    }
                }

                // Event forwarding pump
                if let Some(mut rx) = engine.take_event_receiver() {
                    while let Some(event) = rx.recv().await {
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
                            sayl_core::event::EventPayload::StateChanged { to, .. } => {
                                if to.is_terminal() || *to == DownloadState::Paused {
                                    if let Ok(mut stats) = live_stats.lock() {
                                        stats.remove(&task_id);
                                    }
                                }
                            }
                            _ => {}
                        }

                        // Emit general task event
                        let _ = app_handle.emit("sayl://task", &event);
                    }
                }
            });

            Ok(())
        });

    builder
        .run(tauri::generate_context!())
        .expect("error while running SAYL desktop application");
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

        let view = TaskView::from_record(&record, Some(1048576), Some(120));
        assert_eq!(view.id, record.id.to_string());
        assert_eq!(view.url, "https://example.com/archive.zip");
        assert_eq!(view.filename, "archive.zip");
        assert_eq!(view.state, "Queued");
        assert_eq!(view.written, 0);
        // Because state is Queued (not Downloading/Probing), transient speed/eta should be None
        assert_eq!(view.speed_bps, None);
        assert_eq!(view.eta_secs, None);
        assert_eq!(view.error, None);
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

        let view = TaskView::from_record(&record, Some(2_000_000), Some(25));
        assert_eq!(view.state, "Downloading");
        assert_eq!(view.speed_bps, Some(2_000_000));
        assert_eq!(view.eta_secs, Some(25));
        assert_eq!(view.written, 50_000_000);
        assert_eq!(view.total_size, Some(100_000_000));
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
        let decoded: ProgressPayload = serde_json::from_str(&json).expect("deserialization succeeds");
        assert_eq!(p, decoded);
    }
}
