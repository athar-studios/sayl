//! Terminal progress bar and formatting helpers for streaming download status to stderr.

use std::io::Write;
use sayl_core::event::{Event, EventPayload};
use sayl_core::id::TaskId;

/// Formats a byte quantity into human-readable binary units (KiB, MiB, GiB, TiB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let b = bytes as f64;
    if b >= TB {
        format!("{:.2} TiB", b / TB)
    } else if b >= GB {
        format!("{:.2} GiB", b / GB)
    } else if b >= MB {
        format!("{:.2} MiB", b / MB)
    } else if b >= KB {
        format!("{:.2} KiB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// Formats transfer speed in bytes per second into a human-readable rate.
pub fn format_speed(bps: u64) -> String {
    format!("{}/s", format_bytes(bps))
}

/// Progress tracker that streams progress updates to stderr.
pub struct ProgressTracker {
    target_task_id: TaskId,
    quiet: bool,
    json: bool,
    printed_anything: bool,
}

impl ProgressTracker {
    /// Creates a new progress tracker for a specific task.
    pub fn new(target_task_id: TaskId, quiet: bool, json: bool) -> Self {
        Self {
            target_task_id,
            quiet,
            json,
            printed_anything: false,
        }
    }

    /// Handles an incoming engine event and updates stderr accordingly.
    pub fn handle_event(&mut self, event: &Event) {
        if self.quiet || self.json {
            return;
        }
        if event.task_id != self.target_task_id {
            return;
        }

        match &event.payload {
            EventPayload::Progress {
                downloaded_bytes,
                total_bytes,
                speed_bps,
            } => {
                let speed_str = format_speed(*speed_bps);
                let down_str = format_bytes(*downloaded_bytes);

                if let Some(total) = *total_bytes {
                    let total_str = format_bytes(total);
                    let pct = if total > 0 {
                        (*downloaded_bytes as f64 / total as f64 * 100.0).min(100.0)
                    } else {
                        0.0
                    };
                    eprint!("\r[{:>5.1}%] {} / {} ({})   ", pct, down_str, total_str, speed_str);
                } else {
                    eprint!("\r{} ({})   ", down_str, speed_str);
                }
                let _ = std::io::stderr().flush();
                self.printed_anything = true;
            }
            EventPayload::Completed { total_bytes, .. } => {
                let size_str = format_bytes(*total_bytes);
                eprintln!("\r[100.0%] {} / {} (completed)   ", size_str, size_str);
                let _ = std::io::stderr().flush();
                self.printed_anything = false;
            }
            EventPayload::StateChanged { to, .. } => {
                eprintln!("\rStatus: {}   ", to);
                let _ = std::io::stderr().flush();
            }
            _ => {}
        }
    }

    /// Flushes any pending line breaks.
    pub fn finish(&mut self) {
        if self.printed_anything && !self.quiet && !self.json {
            eprintln!();
            let _ = std::io::stderr().flush();
            self.printed_anything = false;
        }
    }
}
