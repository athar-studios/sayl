use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

pub const BRAND_ICON_BYTES: &[u8] = include_bytes!("../../../../assets/brand/icon-32.png");

/// Formats a transfer rate in bytes per second to a human-readable string.
pub fn format_speed_short(speed_bps: u64) -> String {
    if speed_bps < 1024 {
        format!("{speed_bps} B")
    } else if speed_bps < 1024 * 1024 {
        format!("{:.1} KB", speed_bps as f64 / 1024.0)
    } else if speed_bps < 1024 * 1024 * 1024 {
        format!("{:.1} MB", speed_bps as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", speed_bps as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Formats the tray tooltip string according to the user language.
///
/// Arabic: `سيل — <speed>/s — <n> تنزيلات`
/// English: `SAYL — <speed>/s — <n> downloads`
pub fn format_tooltip(language: &str, total_speed_bps: u64, active_count: usize) -> String {
    let speed_str = format_speed_short(total_speed_bps);
    if language == "en" {
        let plural = if active_count == 1 {
            "download"
        } else {
            "downloads"
        };
        format!("SAYL — {speed_str}/s — {active_count} {plural}")
    } else {
        format!("سيل — {speed_str}/s — {active_count} تنزيلات")
    }
}

/// Builds and registers the application tray icon and context menu.
pub fn setup_tray(app: &AppHandle) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let lang = app
        .try_state::<crate::AppState>()
        .and_then(|s| {
            s.settings
                .read()
                .ok()
                .map(|settings| settings.language.clone())
        })
        .unwrap_or_else(|| "ar".to_string());

    let (open_label, add_label, pause_label, resume_label, quit_label) = if lang == "en" {
        (
            "Open SAYL",
            "Add Download",
            "Pause All",
            "Resume All",
            "Quit",
        )
    } else {
        (
            "فتح سيل",
            "إضافة تنزيل",
            "إيقاف الكل",
            "استئناف الكل",
            "خروج",
        )
    };

    let menu = MenuBuilder::new(app)
        .text("open", open_label)
        .text("add", add_label)
        .separator()
        .text("pause_all", pause_label)
        .text("resume_all", resume_label)
        .separator()
        .text("quit", quit_label)
        .build()?;

    let icon = tauri::image::Image::from_bytes(BRAND_ICON_BYTES)?;
    let initial_tooltip = format_tooltip(&lang, 0, 0);

    let tray = TrayIconBuilder::with_id("sayl-tray")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip(initial_tooltip)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
            "add" => {
                let _ = app.emit("sayl://tray-action", "add");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
            "pause_all" => {
                let _ = app.emit("sayl://tray-action", "pause-all");
            }
            "resume_all" => {
                let _ = app.emit("sayl://tray-action", "resume-all");
            }
            "quit" => {
                crate::perf::log_event("exit");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(tray)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_speed_short() {
        assert_eq!(format_speed_short(500), "500 B");
        assert_eq!(format_speed_short(1024), "1.0 KB");
        assert_eq!(format_speed_short(1_572_864), "1.5 MB");
        assert_eq!(format_speed_short(2_147_483_648), "2.00 GB");
    }

    #[test]
    fn test_format_tooltip_ar_and_en() {
        let ar = format_tooltip("ar", 2_097_152, 3);
        assert_eq!(ar, "سيل — 2.0 MB/s — 3 تنزيلات");

        let en_plural = format_tooltip("en", 1024, 2);
        assert_eq!(en_plural, "SAYL — 1.0 KB/s — 2 downloads");

        let en_single = format_tooltip("en", 1024, 1);
        assert_eq!(en_single, "SAYL — 1.0 KB/s — 1 download");
    }

    #[test]
    fn test_brand_icon_bytes_valid() {
        assert!(!BRAND_ICON_BYTES.is_empty());
        // PNG header check
        assert_eq!(&BRAND_ICON_BYTES[0..8], b"\x89PNG\r\n\x1a\n");
    }
}
