//! Automated shutdown management for SAYL.
//!
//! Provides a configurable countdown mechanism to power down the machine once
//! all downloads finish, with safety checks that immediately block or abort
//! shutdown if any task fails, pauses, or remains active.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sayl_core::state::DownloadState;
use serde::{Deserialize, Serialize};

/// Default shutdown countdown duration in seconds.
pub const SHUTDOWN_COUNTDOWN_SECS: u64 = 60;

/// Live status of the shutdown controller emitted over `sayl://shutdown`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShutdownStatus {
    pub armed: bool,
    pub seconds_left: Option<u64>,
    pub blocked_reason: Option<String>,
}

/// Evaluation result determining whether machine shutdown may proceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownDecision {
    Proceed,
    Blocked(&'static str),
}

/// Evaluates a slice of task states to determine if shutdown is safe.
///
/// Priority:
/// 1. Empty task list -> `Blocked("no_tasks")`
/// 2. Any Failed -> `Blocked("failed_download")`
/// 3. Any Paused or Pausing -> `Blocked("paused_download")`
/// 4. Any active task (Queued, Probing, Downloading, Verifying, Finalizing, Cancelling) -> `Blocked("active_download")`
/// 5. Otherwise (all tasks Completed or Cancelled) -> `Proceed`
pub fn shutdown_decision(states: &[DownloadState]) -> ShutdownDecision {
    if states.is_empty() {
        return ShutdownDecision::Blocked("no_tasks");
    }
    if states.contains(&DownloadState::Failed) {
        return ShutdownDecision::Blocked("failed_download");
    }
    if states.contains(&DownloadState::Paused) || states.contains(&DownloadState::Pausing) {
        return ShutdownDecision::Blocked("paused_download");
    }
    if states.iter().any(|s| {
        matches!(
            *s,
            DownloadState::Queued
                | DownloadState::Probing
                | DownloadState::Downloading
                | DownloadState::Verifying
                | DownloadState::Finalizing
                | DownloadState::Cancelling
        )
    }) {
        return ShutdownDecision::Blocked("active_download");
    }
    ShutdownDecision::Proceed
}

/// Trait abstracting execution of the operating system shutdown command.
pub trait ShutdownRunner: Send + Sync {
    fn run(&self) -> Result<(), String>;
}

/// Production shutdown runner that invokes Windows `shutdown /s /t 0`.
pub struct SystemShutdownRunner;

impl ShutdownRunner for SystemShutdownRunner {
    fn run(&self) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let mut cmd = std::process::Command::new("shutdown");
            cmd.args(["/s", "/t", "0"]);
            cmd.creation_flags(0x08000000);
            cmd.spawn()
                .map_err(|e| format!("Failed to execute shutdown command: {e}"))?;
            Ok(())
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err("System shutdown is only supported on Windows".to_string())
        }
    }
}

/// In-memory controller managing arming, countdown ticking, cancellation, and execution.
pub struct ShutdownController {
    runner: Arc<dyn ShutdownRunner>,
    states: Arc<dyn Fn() -> Vec<DownloadState> + Send + Sync>,
    notify: Arc<dyn Fn(ShutdownStatus) + Send + Sync>,
    generation: Arc<AtomicU64>,
    state: Arc<Mutex<ShutdownStatus>>,
}

impl ShutdownController {
    /// Creates a new `ShutdownController`.
    pub fn new(
        runner: Arc<dyn ShutdownRunner>,
        states: Arc<dyn Fn() -> Vec<DownloadState> + Send + Sync>,
        notify: Arc<dyn Fn(ShutdownStatus) + Send + Sync>,
    ) -> Self {
        Self {
            runner,
            states,
            notify,
            generation: Arc::new(AtomicU64::new(0)),
            state: Arc::new(Mutex::new(ShutdownStatus {
                armed: false,
                seconds_left: None,
                blocked_reason: None,
            })),
        }
    }

    /// Arms the shutdown countdown.
    ///
    /// If the decision is `Blocked`, refuses immediately with the blocked reason.
    /// Otherwise, initiates a 1-second interval countdown that emits status updates,
    /// re-checks the decision when reaching zero, and executes the runner only if still allowed.
    pub fn arm(&self, seconds: Option<u64>) -> ShutdownStatus {
        let decision = shutdown_decision(&(self.states)());
        if let ShutdownDecision::Blocked(reason) = decision {
            let status = ShutdownStatus {
                armed: false,
                seconds_left: None,
                blocked_reason: Some(reason.to_string()),
            };
            self.generation.fetch_add(1, Ordering::SeqCst);
            if let Ok(mut s) = self.state.lock() {
                *s = status.clone();
            }
            (self.notify)(status.clone());
            return status;
        }

        let total_secs = seconds.unwrap_or(SHUTDOWN_COUNTDOWN_SECS);
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let initial_status = ShutdownStatus {
            armed: true,
            seconds_left: Some(total_secs),
            blocked_reason: None,
        };
        if let Ok(mut s) = self.state.lock() {
            *s = initial_status.clone();
        }
        (self.notify)(initial_status.clone());

        if total_secs == 0 {
            let decision = shutdown_decision(&(self.states)());
            match decision {
                ShutdownDecision::Proceed => {
                    let final_status = ShutdownStatus {
                        armed: false,
                        seconds_left: None,
                        blocked_reason: None,
                    };
                    if let Ok(mut s) = self.state.lock() {
                        *s = final_status.clone();
                    }
                    (self.notify)(final_status);
                    let _ = self.runner.run();
                }
                ShutdownDecision::Blocked(reason) => {
                    let blocked_status = ShutdownStatus {
                        armed: false,
                        seconds_left: None,
                        blocked_reason: Some(reason.to_string()),
                    };
                    if let Ok(mut s) = self.state.lock() {
                        *s = blocked_status.clone();
                    }
                    (self.notify)(blocked_status);
                }
            }
            return self.status();
        }

        let runner = Arc::clone(&self.runner);
        let states = Arc::clone(&self.states);
        let notify = Arc::clone(&self.notify);
        let generation = Arc::clone(&self.generation);
        let state = Arc::clone(&self.state);

        tokio::spawn(async move {
            for remaining in (0..total_secs).rev() {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                if generation.load(Ordering::SeqCst) != gen {
                    return;
                }

                if remaining > 0 {
                    let current_status = ShutdownStatus {
                        armed: true,
                        seconds_left: Some(remaining),
                        blocked_reason: None,
                    };
                    if let Ok(mut s) = state.lock() {
                        *s = current_status.clone();
                    }
                    notify(current_status);
                } else {
                    let decision = shutdown_decision(&states());
                    match decision {
                        ShutdownDecision::Proceed => {
                            let final_status = ShutdownStatus {
                                armed: false,
                                seconds_left: None,
                                blocked_reason: None,
                            };
                            if let Ok(mut s) = state.lock() {
                                *s = final_status.clone();
                            }
                            notify(final_status);
                            let _ = runner.run();
                        }
                        ShutdownDecision::Blocked(reason) => {
                            let blocked_status = ShutdownStatus {
                                armed: false,
                                seconds_left: None,
                                blocked_reason: Some(reason.to_string()),
                            };
                            if let Ok(mut s) = state.lock() {
                                *s = blocked_status.clone();
                            }
                            notify(blocked_status);
                        }
                    }
                }
            }
        });

        initial_status
    }

    /// Reliably stops an ongoing shutdown countdown.
    pub fn cancel(&self) -> ShutdownStatus {
        self.generation.fetch_add(1, Ordering::SeqCst);
        let status = ShutdownStatus {
            armed: false,
            seconds_left: None,
            blocked_reason: None,
        };
        if let Ok(mut s) = self.state.lock() {
            *s = status.clone();
        }
        (self.notify)(status.clone());
        status
    }

    /// Returns the current in-memory shutdown status.
    pub fn status(&self) -> ShutdownStatus {
        self.state
            .lock()
            .map(|s| s.clone())
            .unwrap_or(ShutdownStatus {
                armed: false,
                seconds_left: None,
                blocked_reason: None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[derive(Default)]
    struct RecorderRunner {
        calls: AtomicUsize,
    }

    impl ShutdownRunner for RecorderRunner {
        fn run(&self) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn test_shutdown_decision_logic() {
        // empty -> Blocked("no_tasks")
        assert_eq!(
            shutdown_decision(&[]),
            ShutdownDecision::Blocked("no_tasks")
        );

        // all Completed -> Proceed
        assert_eq!(
            shutdown_decision(&[DownloadState::Completed, DownloadState::Completed]),
            ShutdownDecision::Proceed
        );

        // Completed and Cancelled -> Proceed
        assert_eq!(
            shutdown_decision(&[DownloadState::Completed, DownloadState::Cancelled]),
            ShutdownDecision::Proceed
        );

        // any Failed -> Blocked("failed_download")
        assert_eq!(
            shutdown_decision(&[DownloadState::Completed, DownloadState::Failed]),
            ShutdownDecision::Blocked("failed_download")
        );

        // any Paused / Pausing -> Blocked("paused_download")
        assert_eq!(
            shutdown_decision(&[DownloadState::Completed, DownloadState::Paused]),
            ShutdownDecision::Blocked("paused_download")
        );
        assert_eq!(
            shutdown_decision(&[DownloadState::Completed, DownloadState::Pausing]),
            ShutdownDecision::Blocked("paused_download")
        );

        // any active download -> Blocked("active_download")
        assert_eq!(
            shutdown_decision(&[DownloadState::Completed, DownloadState::Downloading]),
            ShutdownDecision::Blocked("active_download")
        );
        assert_eq!(
            shutdown_decision(&[DownloadState::Queued]),
            ShutdownDecision::Blocked("active_download")
        );
        assert_eq!(
            shutdown_decision(&[DownloadState::Probing]),
            ShutdownDecision::Blocked("active_download")
        );
        assert_eq!(
            shutdown_decision(&[DownloadState::Verifying]),
            ShutdownDecision::Blocked("active_download")
        );
        assert_eq!(
            shutdown_decision(&[DownloadState::Finalizing]),
            ShutdownDecision::Blocked("active_download")
        );
        assert_eq!(
            shutdown_decision(&[DownloadState::Cancelling]),
            ShutdownDecision::Blocked("active_download")
        );
    }

    #[tokio::test]
    async fn test_shutdown_controller_one_second_countdown_executes_once() {
        let runner = Arc::new(RecorderRunner::default());
        let states: Arc<dyn Fn() -> Vec<DownloadState> + Send + Sync> =
            Arc::new(|| vec![DownloadState::Completed]);
        let notify = Arc::new(|_status| {});

        let controller = ShutdownController::new(Arc::clone(&runner) as _, states, notify);

        let status = controller.arm(Some(1));
        assert!(status.armed);
        assert_eq!(status.seconds_left, Some(1));

        tokio::time::sleep(tokio::time::Duration::from_millis(1150)).await;
        assert_eq!(runner.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_shutdown_controller_blocked_states_never_execute() {
        let runner = Arc::new(RecorderRunner::default());
        let states: Arc<dyn Fn() -> Vec<DownloadState> + Send + Sync> =
            Arc::new(|| vec![DownloadState::Downloading]);
        let notify = Arc::new(|_status| {});

        let controller = ShutdownController::new(Arc::clone(&runner) as _, states, notify);

        let status = controller.arm(Some(1));
        assert!(!status.armed);
        assert_eq!(status.blocked_reason, Some("active_download".to_string()));

        tokio::time::sleep(tokio::time::Duration::from_millis(1150)).await;
        assert_eq!(runner.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_shutdown_controller_cancel_before_zero_never_executes() {
        let runner = Arc::new(RecorderRunner::default());
        let states: Arc<dyn Fn() -> Vec<DownloadState> + Send + Sync> =
            Arc::new(|| vec![DownloadState::Completed]);
        let notify = Arc::new(|_status| {});

        let controller = ShutdownController::new(Arc::clone(&runner) as _, states, notify);

        let status = controller.arm(Some(1));
        assert!(status.armed);

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let cancel_status = controller.cancel();
        assert!(!cancel_status.armed);

        tokio::time::sleep(tokio::time::Duration::from_millis(1150)).await;
        assert_eq!(runner.calls.load(Ordering::SeqCst), 0);
    }
}
