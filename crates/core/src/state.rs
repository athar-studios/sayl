//! Core download state machine and transition logic.
//!
//! Implements the 11 download states and explicit, guarded state transitions
//! defined in `docs/MASTER-PLAN.md` §7.

use std::fmt;

/// All eleven discrete lifecycle states of a download task.
///
/// State flow follows the architecture specification:
/// ```text
/// Queued -> Probing -> Downloading -> Verifying -> Finalizing -> Completed
/// Downloading -> Pausing -> Paused
/// Paused -> Probing -> Downloading
/// nonterminal -> Cancelling -> Cancelled
/// nonterminal -> Failed
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DownloadState {
    /// Task has been registered and is waiting in the scheduler queue.
    Queued,
    /// Probing the remote origin (HEAD or ranged GET) for size, ranges, ETag.
    Probing,
    /// Actively receiving body bytes across one or more worker connections.
    Downloading,
    /// Verifying checksums or hash integrity after byte reception.
    Verifying,
    /// Flushing buffers, syncing durable sqlite coverage, and renaming target file.
    Finalizing,
    /// Successfully finished and verified; terminal state.
    Completed,
    /// Quiescing workers, closing ranges, and preparing to persist paused coverage.
    Pausing,
    /// Dormant state with durable partial coverage; can resume back to Probing.
    Paused,
    /// In the process of aborting active subrequests and releasing segment leases.
    Cancelling,
    /// Task was cancelled by user or scheduler; terminal state.
    Cancelled,
    /// Task encountered an unrecoverable or retry-exhausted error; terminal state.
    Failed,
}

impl DownloadState {
    /// Returns an array containing all eleven download states.
    pub const ALL: [DownloadState; 11] = [
        DownloadState::Queued,
        DownloadState::Probing,
        DownloadState::Downloading,
        DownloadState::Verifying,
        DownloadState::Finalizing,
        DownloadState::Completed,
        DownloadState::Pausing,
        DownloadState::Paused,
        DownloadState::Cancelling,
        DownloadState::Cancelled,
        DownloadState::Failed,
    ];

    /// Returns `true` if this state is terminal (no further transitions permitted).
    ///
    /// Terminal states are: [`Completed`](Self::Completed), [`Cancelled`](Self::Cancelled), and [`Failed`](Self::Failed).
    #[inline]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }

    /// Returns `true` if the task is actively consuming compute, network, or disk resources.
    #[inline]
    pub const fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Probing
                | Self::Downloading
                | Self::Verifying
                | Self::Finalizing
                | Self::Pausing
                | Self::Cancelling
        )
    }

    /// Returns `true` if this state can be paused by user or policy request.
    #[inline]
    pub const fn can_pause(&self) -> bool {
        matches!(self, Self::Probing | Self::Downloading)
    }

    /// Returns `true` if this state can be resumed.
    #[inline]
    pub const fn can_resume(&self) -> bool {
        matches!(self, Self::Paused)
    }

    /// Returns `true` if this state can be cancelled.
    #[inline]
    pub const fn can_cancel(&self) -> bool {
        !self.is_terminal()
    }

    /// Returns a slice of valid destination states from the current state.
    pub const fn valid_destinations(&self) -> &'static [DownloadState] {
        match self {
            Self::Queued => &[Self::Probing, Self::Cancelling, Self::Failed],
            Self::Probing => &[Self::Downloading, Self::Pausing, Self::Cancelling, Self::Failed],
            Self::Downloading => &[Self::Verifying, Self::Pausing, Self::Cancelling, Self::Failed],
            Self::Verifying => &[Self::Finalizing, Self::Cancelling, Self::Failed],
            Self::Finalizing => &[Self::Completed, Self::Cancelling, Self::Failed],
            Self::Pausing => &[Self::Paused, Self::Cancelling, Self::Failed],
            Self::Paused => &[Self::Probing, Self::Cancelling, Self::Failed],
            Self::Cancelling => &[Self::Cancelled, Self::Failed],
            Self::Completed => &[],
            Self::Cancelled => &[],
            Self::Failed => &[],
        }
    }

    /// Returns a static string representation of the state name.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Probing => "Probing",
            Self::Downloading => "Downloading",
            Self::Verifying => "Verifying",
            Self::Finalizing => "Finalizing",
            Self::Completed => "Completed",
            Self::Pausing => "Pausing",
            Self::Paused => "Paused",
            Self::Cancelling => "Cancelling",
            Self::Cancelled => "Cancelled",
            Self::Failed => "Failed",
        }
    }
}

impl fmt::Display for DownloadState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when an invalid state transition is attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateTransitionError {
    /// Origin state of the attempted transition.
    pub from: DownloadState,
    /// Destination state of the attempted transition.
    pub to: DownloadState,
}

impl fmt::Display for StateTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid state transition from {} to {}",
            self.from, self.to
        )
    }
}

impl std::error::Error for StateTransitionError {}

/// Attempts an explicit transition from `from` state to `to` state.
///
/// Returns `Ok(to)` if the transition is allowed by the architecture state machine,
/// or `Err(StateTransitionError)` if the transition is invalid or rejected.
///
/// # Examples
/// ```
/// use sayl_core::{DownloadState, transition};
///
/// // Valid forward progression
/// assert_eq!(transition(DownloadState::Queued, DownloadState::Probing), Ok(DownloadState::Probing));
///
/// // Terminal state cannot transition anywhere
/// assert!(transition(DownloadState::Completed, DownloadState::Queued).is_err());
/// ```
pub fn transition(from: DownloadState, to: DownloadState) -> Result<DownloadState, StateTransitionError> {
    if from == to {
        return if !from.is_terminal() {
            Ok(from)
        } else {
            Err(StateTransitionError { from, to })
        };
    }

    let destinations = from.valid_destinations();
    if destinations.contains(&to) {
        Ok(to)
    } else {
        Err(StateTransitionError { from, to })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_forward_chain() {
        let chain = [
            (DownloadState::Queued, DownloadState::Probing),
            (DownloadState::Probing, DownloadState::Downloading),
            (DownloadState::Downloading, DownloadState::Verifying),
            (DownloadState::Verifying, DownloadState::Finalizing),
            (DownloadState::Finalizing, DownloadState::Completed),
        ];

        for (from, to) in chain {
            assert_eq!(transition(from, to), Ok(to));
        }
    }

    #[test]
    fn test_pause_and_resume_cycle() {
        assert_eq!(transition(DownloadState::Downloading, DownloadState::Pausing), Ok(DownloadState::Pausing));
        assert_eq!(transition(DownloadState::Pausing, DownloadState::Paused), Ok(DownloadState::Paused));
        assert_eq!(transition(DownloadState::Paused, DownloadState::Probing), Ok(DownloadState::Probing));
        assert_eq!(transition(DownloadState::Probing, DownloadState::Downloading), Ok(DownloadState::Downloading));
    }

    #[test]
    fn test_probe_pause() {
        assert_eq!(transition(DownloadState::Probing, DownloadState::Pausing), Ok(DownloadState::Pausing));
    }

    #[test]
    fn test_cancellation_paths() {
        for state in DownloadState::ALL {
            if !state.is_terminal() {
                assert_eq!(
                    transition(state, DownloadState::Cancelling),
                    Ok(DownloadState::Cancelling),
                    "State {:?} should allow transition to Cancelling",
                    state
                );
            }
        }
        assert_eq!(transition(DownloadState::Cancelling, DownloadState::Cancelled), Ok(DownloadState::Cancelled));
    }

    #[test]
    fn test_failure_paths() {
        for state in DownloadState::ALL {
            if !state.is_terminal() {
                assert_eq!(
                    transition(state, DownloadState::Failed),
                    Ok(DownloadState::Failed),
                    "State {:?} should allow transition to Failed",
                    state
                );
            }
        }
    }

    // Property-based test over all 121 transitions
    #[test]
    fn test_exhaustive_transition_matrix_properties() {
        let all = DownloadState::ALL;
        let mut valid_count = 0;
        let mut invalid_count = 0;

        for &from in &all {
            for &to in &all {
                let res = transition(from, to);

                // Property 1: Self-transitions are valid only for nonterminal states and invalid for terminal states
                if from == to {
                    if from.is_terminal() {
                        assert!(
                            res.is_err(),
                            "Terminal self-transition from {:?} to {:?} must never be allowed",
                            from,
                            to
                        );
                    } else {
                        assert!(
                            res.is_ok(),
                            "Nonterminal self-transition from {:?} to {:?} must be allowed",
                            from,
                            to
                        );
                    }
                }

                // Property 2: Terminal states can NEVER transition to any state
                if from.is_terminal() {
                    assert!(
                        res.is_err(),
                        "Terminal state {:?} must never transition to {:?}",
                        from,
                        to
                    );
                }

                // Property 3: Cancelling can only transition to Cancelled or Failed (or Cancelling self-transition)
                if from == DownloadState::Cancelling {
                    if to == DownloadState::Cancelled || to == DownloadState::Failed || to == DownloadState::Cancelling {
                        assert!(res.is_ok());
                    } else {
                        assert!(res.is_err());
                    }
                }

                // Property 4: All nonterminal states can transition to Cancelling and Failed
                if !from.is_terminal() {
                    if to == DownloadState::Cancelling || to == DownloadState::Failed {
                        assert!(
                            res.is_ok(),
                            "Nonterminal {:?} must be able to transition to {:?}",
                            from,
                            to
                        );
                    }
                }

                if res.is_ok() {
                    valid_count += 1;
                } else {
                    invalid_count += 1;
                    let err = res.unwrap_err();
                    assert_eq!(err.from, from);
                    assert_eq!(err.to, to);
                }
            }
        }

        assert_eq!(valid_count + invalid_count, 121);
        // Valid transitions:
        // Queued: 3 (Probing, Cancelling, Failed)
        // Probing: 4 (Downloading, Pausing, Cancelling, Failed)
        // Downloading: 4 (Verifying, Pausing, Cancelling, Failed)
        // Verifying: 3 (Finalizing, Cancelling, Failed)
        // Finalizing: 3 (Completed, Cancelling, Failed)
        // Pausing: 3 (Paused, Cancelling, Failed)
        // Paused: 3 (Probing, Cancelling, Failed)
        // Cancelling: 2 (Cancelled, Failed)
        // Completed: 0
        // Cancelled: 0
        // Failed: 0
        // Total valid = 3 + 4 + 4 + 3 + 3 + 3 + 3 + 2 = 25
        // Plus 8 nonterminal self-loops (Queued, Probing, Downloading, Verifying, Finalizing, Pausing, Paused, Cancelling) = 33
        assert_eq!(valid_count, 33);
        assert_eq!(invalid_count, 88);
    }

    #[test]
    fn test_reachability_of_completed() {
        // Verify path existence: Queued -> Probing -> Downloading -> Verifying -> Finalizing -> Completed
        let mut current = DownloadState::Queued;
        let expected_path = [
            DownloadState::Probing,
            DownloadState::Downloading,
            DownloadState::Verifying,
            DownloadState::Finalizing,
            DownloadState::Completed,
        ];

        for &next in &expected_path {
            current = transition(current, next).expect("step should be valid");
        }
        assert_eq!(current, DownloadState::Completed);
        assert!(current.is_terminal());
    }
}
