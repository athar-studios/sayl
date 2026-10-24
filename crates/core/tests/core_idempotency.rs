//! Integration tests for idempotency keys and deduplication storage.

use sayl_core::{IdempotencyKey, IdempotencyStore, TaskId};
use std::time::Duration;

#[test]
fn test_idempotent_command_deduplication() {
    let store = IdempotencyStore::new(Duration::from_secs(3600));
    let key = IdempotencyKey::new();
    let task_id = TaskId::new();

    // First execution: key not found
    assert!(store.get(&key).is_none());

    // Execute command and record outcome
    let response = "task-started-successfully".to_string();
    store.insert(key, task_id, response.clone());

    // Second execution with identical key returns cached response
    let cached = store.get(&key).expect("cached record must be present");
    assert_eq!(cached.key, key);
    assert_eq!(cached.task_id, task_id);
    assert_eq!(cached.response, response);
}

#[test]
fn test_concurrent_keys_for_different_tasks() {
    let store = IdempotencyStore::new(Duration::from_secs(3600));

    let key1 = IdempotencyKey::new();
    let task1 = TaskId::new();

    let key2 = IdempotencyKey::new();
    let task2 = TaskId::new();

    store.insert(key1, task1, "pause-task-1".to_string());
    store.insert(key2, task2, "cancel-task-2".to_string());

    assert_eq!(store.len(), 2);

    let rec1 = store.get(&key1).expect("key1 found");
    assert_eq!(rec1.task_id, task1);
    assert_eq!(rec1.response, "pause-task-1");

    let rec2 = store.get(&key2).expect("key2 found");
    assert_eq!(rec2.task_id, task2);
    assert_eq!(rec2.response, "cancel-task-2");
}

#[test]
fn test_store_cleanup_lifecycle() {
    let store = IdempotencyStore::new(Duration::from_millis(20));
    let task_id = TaskId::new();

    let k1 = IdempotencyKey::new();
    let k2 = IdempotencyKey::new();

    store.insert(k1, task_id, "resp1".to_string());
    store.insert(k2, task_id, "resp2".to_string());
    assert_eq!(store.len(), 2);

    // Wait past the 20ms TTL
    std::thread::sleep(Duration::from_millis(30));

    // Both keys should now be treated as expired by get()
    assert!(store.get(&k1).is_none());
    assert!(store.get(&k2).is_none());

    // Explicit cleanup should purge both entries
    let purged = store.cleanup();
    assert_eq!(purged, 2);
    assert!(store.is_empty());
}
