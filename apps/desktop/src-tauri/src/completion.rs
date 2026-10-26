//! IDM-style download completion UI: native completion window and Windows toast.

use sayl_app::Engine;
use sayl_core::id::TaskId;
use sayl_core::state::DownloadState;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::Settings;

/// Decides whether to show the completion window, a toast, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionUiPlan {
    pub show_window: bool,
    pub show_toast: bool,
}

/// Pure policy: toast when the completion window setting is off or the main window is hidden (tray).
pub fn plan_completion_ui(
    show_complete_window: bool,
    main_window_visible: bool,
) -> CompletionUiPlan {
    let in_tray = !main_window_visible;
    let show_window = show_complete_window && !in_tray;
    let show_toast = !show_complete_window || in_tray;
    CompletionUiPlan {
        show_window,
        show_toast,
    }
}

fn main_window_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false)
}

fn format_bytes_simple(bytes: u64, language: &str) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if language == "en" {
        if b >= GB {
            return format!("{:.2} GB", b / GB);
        }
        if b >= MB {
            return format!("{:.2} MB", b / MB);
        }
        if b >= KB {
            return format!("{:.1} KB", b / KB);
        }
        return format!("{bytes} B");
    }
    if b >= GB {
        return format!("{:.2} ج.ب", b / GB);
    }
    if b >= MB {
        return format!("{:.2} م.ب", b / MB);
    }
    if b >= KB {
        return format!("{:.1} ك.ب", b / KB);
    }
    format!("{bytes} بايت")
}

fn notification_copy(language: &str, filename: &str, bytes: u64) -> (String, String) {
    let size = format_bytes_simple(bytes, language);
    if language == "en" {
        (
            "Download complete".to_string(),
            format!("{filename} ({size})"),
        )
    } else {
        ("اكتمل التحميل".to_string(), format!("{filename} ({size})"))
    }
}

fn window_background(app: &AppHandle, settings: &Settings) -> tauri::window::Color {
    let is_dark = if settings.theme == "dark" {
        true
    } else if settings.theme == "light" {
        false
    } else if let Some(main_win) = app.get_webview_window("main") {
        main_win
            .theme()
            .map(|t| matches!(t, tauri::Theme::Dark))
            .unwrap_or(false)
    } else {
        false
    };
    if is_dark {
        tauri::window::Color(31, 41, 55, 255)
    } else {
        tauri::window::Color(255, 255, 255, 255)
    }
}

pub fn open_complete_window(
    app: &AppHandle,
    task_id: &str,
    settings: &Settings,
) -> Result<(), String> {
    let trimmed = task_id.trim();
    if trimmed.is_empty() {
        return Err("Task ID cannot be empty".into());
    }
    let label = format!("complete-{trimmed}");
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    let title = if settings.language == "en" {
        "Download complete"
    } else {
        "اكتمل التحميل"
    };

    let url = format!("index.html?complete={trimmed}");
    let bg_color = window_background(app, settings);

    tauri::WebviewWindowBuilder::new(app, &label, tauri::WebviewUrl::App(url.into()))
        .title(title)
        .visible(false)
        .inner_size(440.0, 240.0)
        .resizable(false)
        .decorations(true)
        .background_color(bg_color)
        .build()
        .map_err(|e| format!("Failed to create completion window: {e}"))?;
    Ok(())
}

pub fn close_complete_window(app: &AppHandle, task_id: &str) {
    let label = format!("complete-{}", task_id.trim());
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.close();
    }
}

fn show_completion_toast(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Called when a task transitions to `Completed` (live download, not startup recovery).
pub fn handle_download_completed(app: AppHandle, engine: Engine, task_id: TaskId) {
    tauri::async_runtime::spawn(async move {
        let record = match engine.get_task(task_id) {
            Ok(Some(rec)) if rec.state == DownloadState::Completed => rec,
            _ => return,
        };

        let (show_complete_window, language) = app
            .try_state::<crate::AppState>()
            .and_then(|state| {
                state
                    .settings
                    .read()
                    .ok()
                    .map(|s| (s.show_download_complete_window, s.language.clone()))
            })
            .unwrap_or((true, "ar".to_string()));

        let settings_snapshot = app
            .try_state::<crate::AppState>()
            .and_then(|state| state.settings.read().ok().map(|s| s.clone()))
            .unwrap_or_default();

        let main_visible = main_window_visible(&app);
        let plan = plan_completion_ui(show_complete_window, main_visible);

        let bytes = record
            .total_size
            .unwrap_or(record.written)
            .max(record.written);
        let (title, body) = notification_copy(&language, &record.filename, bytes);

        if plan.show_window {
            if let Err(e) = open_complete_window(&app, &task_id.to_string(), &settings_snapshot) {
                eprintln!("completion window: {e}");
            }
        }

        if plan.show_toast {
            show_completion_toast(&app, &title, &body);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_shows_window_when_setting_on_and_main_visible() {
        let plan = plan_completion_ui(true, true);
        assert!(plan.show_window);
        assert!(!plan.show_toast);
    }

    #[test]
    fn plan_toast_only_when_setting_off() {
        let plan = plan_completion_ui(false, true);
        assert!(!plan.show_window);
        assert!(plan.show_toast);
    }

    #[test]
    fn plan_toast_when_in_tray_even_if_setting_on() {
        let plan = plan_completion_ui(true, false);
        assert!(!plan.show_window);
        assert!(plan.show_toast);
    }

    #[test]
    fn notification_copy_arabic() {
        let (title, body) = notification_copy("ar", "file.zip", 1024);
        assert_eq!(title, "اكتمل التحميل");
        assert!(body.contains("file.zip"));
    }
}
