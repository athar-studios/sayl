//! Integration tests for the download lifecycle state machine.

use sayl_core::{event_channel, transition, DownloadState, EventKind, EventPayload, TaskId};

#[test]
fn test_complete_download_lifecycle() {
    let task_id = TaskId::new();
    let (event_sender, mut event_receiver) = event_channel(32);

    let mut state = DownloadState::Queued;
    assert!(!state.is_terminal());
    assert!(!state.is_active());

    // Queued -> Probing
    state = transition(state, DownloadState::Probing).expect("Queued -> Probing");
    assert!(state.is_active());
    event_sender
        .try_emit(
            task_id,
            EventKind::StateChanged,
            EventPayload::StateChanged {
                from: DownloadState::Queued,
                to: DownloadState::Probing,
            },
        )
        .expect("emit event");

    // Probing -> Downloading
    state = transition(state, DownloadState::Downloading).expect("Probing -> Downloading");
    assert!(state.is_active());
    assert!(state.can_pause());

    // Downloading -> Verifying
    state = transition(state, DownloadState::Verifying).expect("Downloading -> Verifying");
    assert!(state.is_active());
    assert!(!state.can_pause());

    // Verifying -> Finalizing
    state = transition(state, DownloadState::Finalizing).expect("Verifying -> Finalizing");
    assert!(state.is_active());

    // Finalizing -> Completed
    state = transition(state, DownloadState::Completed).expect("Finalizing -> Completed");
    assert!(state.is_terminal());
    assert!(!state.is_active());
    assert!(!state.can_cancel());

    // Verify terminal invariants: no transitions out of Completed
    for &target in &DownloadState::ALL {
        assert!(
            transition(state, target).is_err(),
            "Completed cannot transition to {:?}",
            target
        );
    }

    // Verify event received
    let event = event_receiver.try_recv().expect("event should be queued");
    assert_eq!(event.id, 1);
    assert_eq!(event.task_id, task_id);
    assert_eq!(event.kind, EventKind::StateChanged);
}

#[test]
fn test_pause_resume_cycle_integration() {
    let mut state = DownloadState::Downloading;

    // Downloading -> Pausing -> Paused
    state = transition(state, DownloadState::Pausing).expect("Downloading -> Pausing");
    assert_eq!(state, DownloadState::Pausing);
    assert!(state.is_active());

    state = transition(state, DownloadState::Paused).expect("Pausing -> Paused");
    assert_eq!(state, DownloadState::Paused);
    assert!(!state.is_active());
    assert!(state.can_resume());
    assert!(!state.is_terminal());

    // Resume: Paused -> Probing -> Downloading
    state = transition(state, DownloadState::Probing).expect("Paused -> Probing");
    assert_eq!(state, DownloadState::Probing);
    assert!(state.is_active());

    state = transition(state, DownloadState::Downloading).expect("Probing -> Downloading");
    assert_eq!(state, DownloadState::Downloading);
}

#[test]
fn test_cancellation_from_any_active_state() {
    let nonterminal_states = [
        DownloadState::Queued,
        DownloadState::Probing,
        DownloadState::Downloading,
        DownloadState::Verifying,
        DownloadState::Finalizing,
        DownloadState::Pausing,
        DownloadState::Paused,
    ];

    for initial in nonterminal_states {
        let cancelling = transition(initial, DownloadState::Cancelling)
            .unwrap_or_else(|_| panic!("Failed to cancel from {:?}", initial));
        assert_eq!(cancelling, DownloadState::Cancelling);

        let cancelled =
            transition(cancelling, DownloadState::Cancelled).expect("Cancelling -> Cancelled");
        assert_eq!(cancelled, DownloadState::Cancelled);
        assert!(cancelled.is_terminal());
    }
}

#[test]
fn test_failure_from_any_nonterminal_state() {
    for initial in DownloadState::ALL {
        if !initial.is_terminal() {
            let failed = transition(initial, DownloadState::Failed)
                .unwrap_or_else(|_| panic!("Failed to fail from {:?}", initial));
            assert_eq!(failed, DownloadState::Failed);
            assert!(failed.is_terminal());
        }
    }
}
