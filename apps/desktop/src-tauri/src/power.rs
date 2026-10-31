//! System sleep prevention management while downloads are active.
//!
//! On Windows, calls `SetThreadExecutionState` from a dedicated thread to ensure
//! the execution state assertion remains active as long as required and is cleanly
//! released when downloads finish or the application exits.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::thread::JoinHandle;

#[cfg(windows)]
const ES_CONTINUOUS: u32 = 0x8000_0000;
#[cfg(windows)]
const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;

#[cfg(windows)]
extern "system" {
    fn SetThreadExecutionState(flags: u32) -> u32;
}

enum PowerCmd {
    SetActive(bool),
    Terminate,
}

/// Pure helper determining if sleep prevention should be active.
///
/// Returns `true` if and only if the user setting is enabled and there is at least one active download.
pub fn should_inhibit(prevent_sleep_enabled: bool, active_downloads: usize) -> bool {
    prevent_sleep_enabled && active_downloads > 0
}

/// Manages system sleep prevention using a dedicated background thread.
pub struct SleepInhibitor {
    tx: Mutex<Option<Sender<PowerCmd>>>,
    thread_handle: Mutex<Option<JoinHandle<()>>>,
    active: AtomicBool,
}

impl SleepInhibitor {
    /// Creates and initializes a new `SleepInhibitor`.
    pub fn new() -> Self {
        let (tx, rx) = channel::<PowerCmd>();

        #[cfg(windows)]
        let thread_handle = std::thread::Builder::new()
            .name("sayl-power-inhibitor".to_string())
            .spawn(move || {
                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        PowerCmd::SetActive(active) => {
                            let flags = if active {
                                ES_CONTINUOUS | ES_SYSTEM_REQUIRED
                            } else {
                                ES_CONTINUOUS
                            };
                            unsafe {
                                SetThreadExecutionState(flags);
                            }
                        }
                        PowerCmd::Terminate => {
                            unsafe {
                                SetThreadExecutionState(ES_CONTINUOUS);
                            }
                            break;
                        }
                    }
                }
                unsafe {
                    SetThreadExecutionState(ES_CONTINUOUS);
                }
            })
            .ok();

        #[cfg(not(windows))]
        let thread_handle = std::thread::Builder::new()
            .name("sayl-power-inhibitor".to_string())
            .spawn(move || {
                while let Ok(cmd) = rx.recv() {
                    if matches!(cmd, PowerCmd::Terminate) {
                        break;
                    }
                }
            })
            .ok();

        Self {
            tx: Mutex::new(Some(tx)),
            thread_handle: Mutex::new(thread_handle),
            active: AtomicBool::new(false),
        }
    }

    /// Sets whether sleep inhibition should be active.
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::SeqCst);
        if let Ok(guard) = self.tx.lock() {
            if let Some(ref sender) = *guard {
                let _ = sender.send(PowerCmd::SetActive(active));
            }
        }
    }

    /// Returns whether sleep inhibition is currently active.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }
}

impl Default for SleepInhibitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SleepInhibitor {
    fn drop(&mut self) {
        self.active.store(false, Ordering::SeqCst);
        let sender = self.tx.lock().ok().and_then(|mut g| g.take());
        if let Some(s) = sender {
            let _ = s.send(PowerCmd::Terminate);
        }
        let handle = self.thread_handle.lock().ok().and_then(|mut g| g.take());
        if let Some(h) = handle {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_inhibit_conditions() {
        // false when disabled, regardless of active count
        assert!(!should_inhibit(false, 0));
        assert!(!should_inhibit(false, 1));
        assert!(!should_inhibit(false, 10));

        // false with zero active, even when enabled
        assert!(!should_inhibit(true, 0));

        // true only when enabled and active > 0
        assert!(should_inhibit(true, 1));
        assert!(should_inhibit(true, 5));
    }

    #[test]
    fn test_sleep_inhibitor_state_lifecycle() {
        let inhibitor = SleepInhibitor::new();
        assert!(!inhibitor.is_active());

        inhibitor.set_active(true);
        assert!(inhibitor.is_active());

        inhibitor.set_active(false);
        assert!(!inhibitor.is_active());
    }
}
