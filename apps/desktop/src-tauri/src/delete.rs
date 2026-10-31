//! Delete operations and Windows Recycle Bin integration.
//!
//! Provides permanent task removal from store and write-ahead journal (`journal.jsonl`),
//! and optional file removal to the Windows Recycle Bin using `SHFileOperationW`.

use sayl_core::id::TaskId;
use std::path::Path;

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(clippy::upper_case_acronyms)]
struct SHFILEOPSTRUCTW {
    hwnd: *mut std::ffi::c_void,
    w_func: u32,
    p_from: *const u16,
    p_to: *const u16,
    f_flags: u16,
    f_any_operations_aborted: i32,
    h_name_mappings: *mut std::ffi::c_void,
    lpsz_progress_title: *const u16,
}

#[cfg(windows)]
const FO_DELETE: u32 = 0x0003;
#[cfg(windows)]
const FOF_ALLOWUNDO: u16 = 0x0040;
#[cfg(windows)]
const FOF_NOCONFIRMATION: u16 = 0x0010;
#[cfg(windows)]
const FOF_SILENT: u16 = 0x0004;
#[cfg(windows)]
const FOF_NOERRORUI: u16 = 0x0400;

#[cfg(windows)]
#[link(name = "shell32")]
extern "system" {
    fn SHFileOperationW(lpFileOp: *mut SHFILEOPSTRUCTW) -> i32;
}

/// Moves a file to the Windows Recycle Bin, or deletes it directly on non-Windows.
///
/// On Windows, invokes `SHFileOperationW` with `FO_DELETE`, `FOF_ALLOWUNDO`,
/// `FOF_NOCONFIRMATION`, `FOF_SILENT`, and `FOF_NOERRORUI`, with double-null-terminated wide paths.
///
/// Missing files do NOT return an error and return `Ok(())`.
pub fn send_to_recycle_bin(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        let mut wide_path: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide_path.push(0);
        wide_path.push(0);

        let mut op = SHFILEOPSTRUCTW {
            hwnd: std::ptr::null_mut(),
            w_func: FO_DELETE,
            p_from: wide_path.as_ptr(),
            p_to: std::ptr::null(),
            f_flags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI,
            f_any_operations_aborted: 0,
            h_name_mappings: std::ptr::null_mut(),
            lpsz_progress_title: std::ptr::null(),
        };

        let ret = unsafe { SHFileOperationW(&mut op) };
        if ret != 0 {
            return Err(format!("SHFileOperationW failed with exit code {ret}"));
        }
        if op.f_any_operations_aborted != 0 {
            return Err("Recycle bin file deletion was aborted".to_string());
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        if path.is_dir() {
            std::fs::remove_dir_all(path).map_err(|e| e.to_string())
        } else {
            std::fs::remove_file(path).map_err(|e| e.to_string())
        }
    }
}

/// Executes deletion of the given tasks from `engine`.
///
/// If a task is running/active or cancellable, it is cancelled first so file handles are released.
/// If `delete_files` is true, the completed file and any `.part` files are sent to the Recycle Bin.
/// The task is permanently removed from the engine store and write-ahead journal (`journal.jsonl`).
pub async fn execute_delete_tasks(
    engine: &sayl_app::Engine,
    ids: &[TaskId],
    delete_files: bool,
) -> Result<Vec<TaskId>, String> {
    execute_delete_tasks_with_recycler(engine, ids, delete_files, send_to_recycle_bin).await
}

/// Internal execution helper accepting a custom file recycler for testing.
pub async fn execute_delete_tasks_with_recycler<F>(
    engine: &sayl_app::Engine,
    ids: &[TaskId],
    delete_files: bool,
    mut recycle_fn: F,
) -> Result<Vec<TaskId>, String>
where
    F: FnMut(&Path) -> Result<(), String>,
{
    let mut deleted = Vec::new();

    for &task_id in ids {
        let record_opt = engine
            .get_task(task_id)
            .map_err(|e| format!("Failed to get task {task_id}: {e}"))?;

        let record = match record_opt {
            Some(r) => r,
            None => continue,
        };

        // 1. If running or cancellable, cancel first before recycling
        if !record.state.is_terminal() {
            let _ = engine.cancel(task_id).await;
            if record.state.is_active() {
                for _ in 0..20 {
                    tokio::task::yield_now().await;
                    let is_active = engine
                        .get_task(task_id)
                        .ok()
                        .flatten()
                        .map(|r| r.state.is_active())
                        .unwrap_or(false);
                    if !is_active {
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
                }
            }
        }

        // 2. If delete_files requested, recycle completed file and any .part file
        if delete_files {
            let final_file = record.save_path.join(&record.filename);
            let part_file = record.save_path.join(format!("{}.part", record.filename));

            if final_file.exists() {
                recycle_fn(&final_file)?;
            }
            if part_file.exists() {
                recycle_fn(&part_file)?;
            }
        }

        // 3. Delete from engine store and journal
        engine
            .delete_task(task_id)
            .map_err(|e| format!("Failed to delete task {task_id} from store: {e}"))?;

        deleted.push(task_id);
    }

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sayl_app::{Engine, EngineConfig};
    use sayl_core::id::TaskId;
    use sayl_core::state::DownloadState;
    use sayl_storage::{Store, TaskRecord};
    use std::fs::File;
    use std::io::Write;
    use std::sync::Arc;
    use tempfile::{tempdir, tempdir_in, TempDir};

    fn test_temp_dir() -> TempDir {
        if let Ok(cargo_tmp) = std::env::var("CARGO_TARGET_TMPDIR") {
            let _ = std::fs::create_dir_all(&cargo_tmp);
            if let Ok(dir) = tempdir_in(&cargo_tmp) {
                return dir;
            }
        }
        let target_tmp = std::path::Path::new("target").join("test-tmp");
        let _ = std::fs::create_dir_all(&target_tmp);
        if let Ok(dir) = tempdir_in(&target_tmp) {
            return dir;
        }
        tempdir().expect("create temp dir")
    }

    fn insert_test_task(
        store: &Store,
        id: TaskId,
        filename: &str,
        dl_dir: &Path,
        state: DownloadState,
    ) -> TaskRecord {
        let mut record =
            TaskRecord::with_id(id, "http://example.com/testfile.bin", dl_dir, filename);
        record.state = state;
        store.upsert_task(&record).expect("upsert test task");
        record
    }

    // Scenario 1: remove-only (delete_files: false): task removed from store & journal, file remains untouched.
    #[tokio::test]
    async fn test_delete_scenario_1_remove_only() {
        let dl_temp = test_temp_dir();
        let store_temp = test_temp_dir();

        let task_id = TaskId::new();
        let filename = "sample1.bin";
        let file_path = dl_temp.path().join(filename);
        {
            let mut f = File::create(&file_path).unwrap();
            f.write_all(b"sample data 1").unwrap();
        }

        let store = Arc::new(Store::open(store_temp.path()).unwrap());
        insert_test_task(
            &store,
            task_id,
            filename,
            dl_temp.path(),
            DownloadState::Paused,
        );
        let engine =
            Engine::with_store(dl_temp.path(), Arc::clone(&store), EngineConfig::default())
                .unwrap();

        // Delete with delete_files = false
        let deleted = execute_delete_tasks(&engine, &[task_id], false)
            .await
            .expect("delete execution should succeed");
        assert_eq!(deleted, vec![task_id]);

        // 1. Task removed from store in memory
        let in_store = engine.get_task(task_id).unwrap();
        assert!(in_store.is_none(), "Task must be removed from store");

        // 2. Task removed from journal (drop engine & store first to release directory lock, then reopen)
        drop(engine);
        drop(store);
        let reopened_store = Store::open(store_temp.path()).unwrap();
        assert!(
            reopened_store.get_task(task_id).unwrap().is_none(),
            "Task must be removed from journal replay"
        );

        // 3. File remains untouched on disk
        assert!(
            file_path.exists(),
            "File must remain untouched when delete_files is false"
        );
    }

    // Scenario 2: remove+file (delete_files: true): task removed from store & journal, file and .part file sent to recycler.
    #[tokio::test]
    async fn test_delete_scenario_2_remove_and_recycle_files() {
        let dl_temp = test_temp_dir();
        let store_temp = test_temp_dir();

        let task_id = TaskId::new();
        let filename = "sample2.bin";
        let final_path = dl_temp.path().join(filename);
        let part_path = dl_temp.path().join(format!("{filename}.part"));
        {
            let mut f1 = File::create(&final_path).unwrap();
            f1.write_all(b"completed data").unwrap();
            let mut f2 = File::create(&part_path).unwrap();
            f2.write_all(b"partial part data").unwrap();
        }

        let store = Arc::new(Store::open(store_temp.path()).unwrap());
        insert_test_task(
            &store,
            task_id,
            filename,
            dl_temp.path(),
            DownloadState::Paused,
        );
        let engine =
            Engine::with_store(dl_temp.path(), Arc::clone(&store), EngineConfig::default())
                .unwrap();

        let mut recycled_paths = Vec::new();
        let deleted = execute_delete_tasks_with_recycler(&engine, &[task_id], true, |path| {
            recycled_paths.push(path.to_path_buf());
            send_to_recycle_bin(path)
        })
        .await
        .expect("delete execution should succeed");

        assert_eq!(deleted, vec![task_id]);
        assert_eq!(
            recycled_paths.len(),
            2,
            "Both final and .part files should be sent to recycler"
        );
        assert!(recycled_paths.contains(&final_path));
        assert!(recycled_paths.contains(&part_path));

        // Both files no longer exist at original locations
        assert!(!final_path.exists(), "Final file must be removed");
        assert!(!part_path.exists(), ".part file must be removed");

        // Removed from store and journal
        assert!(engine.get_task(task_id).unwrap().is_none());
        drop(engine);
        drop(store);
        let reopened_store = Store::open(store_temp.path()).unwrap();
        assert!(reopened_store.get_task(task_id).unwrap().is_none());
    }

    // Scenario 3: running task: actively running download cancelled first, then deleted from store and disk.
    #[tokio::test]
    async fn test_delete_scenario_3_running_task_cancelled_first() {
        let dl_temp = test_temp_dir();
        let store_temp = test_temp_dir();

        let task_id = TaskId::new();
        let filename = "running_sample.bin";
        let part_path = dl_temp.path().join(format!("{filename}.part"));
        {
            let mut f = File::create(&part_path).unwrap();
            f.write_all(b"active chunk stream").unwrap();
        }

        let store = Arc::new(Store::open(store_temp.path()).unwrap());
        insert_test_task(
            &store,
            task_id,
            filename,
            dl_temp.path(),
            DownloadState::Downloading,
        );
        let engine =
            Engine::with_store(dl_temp.path(), Arc::clone(&store), EngineConfig::default())
                .unwrap();

        // Delete with delete_files = true
        let deleted = execute_delete_tasks(&engine, &[task_id], true)
            .await
            .expect("delete running task should succeed");

        assert_eq!(deleted, vec![task_id]);
        assert!(!part_path.exists(), "Part file must be removed");
        assert!(
            engine.get_task(task_id).unwrap().is_none(),
            "Task removed from store"
        );
        drop(engine);
        drop(store);
        let reopened_store = Store::open(store_temp.path()).unwrap();
        assert!(
            reopened_store.get_task(task_id).unwrap().is_none(),
            "Task removed from journal"
        );
    }

    // Scenario 4: completed task: completed download removed from store, completed file recycled.
    #[tokio::test]
    async fn test_delete_scenario_4_completed_task_recycled() {
        let dl_temp = test_temp_dir();
        let store_temp = test_temp_dir();

        let task_id = TaskId::new();
        let filename = "completed_file.bin";
        let final_path = dl_temp.path().join(filename);
        {
            let mut f = File::create(&final_path).unwrap();
            f.write_all(b"finished 100% data").unwrap();
        }

        let store = Arc::new(Store::open(store_temp.path()).unwrap());
        insert_test_task(
            &store,
            task_id,
            filename,
            dl_temp.path(),
            DownloadState::Completed,
        );
        let engine =
            Engine::with_store(dl_temp.path(), Arc::clone(&store), EngineConfig::default())
                .unwrap();

        let deleted = execute_delete_tasks(&engine, &[task_id], true)
            .await
            .expect("delete completed task should succeed");

        assert_eq!(deleted, vec![task_id]);
        assert!(!final_path.exists(), "Completed file must be recycled");
        assert!(engine.get_task(task_id).unwrap().is_none());
        drop(engine);
        drop(store);
        let reopened_store = Store::open(store_temp.path()).unwrap();
        assert!(reopened_store.get_task(task_id).unwrap().is_none());
    }

    // Scenario 5: missing file: delete_files: true but file doesn't exist on disk; deletion succeeds cleanly.
    #[tokio::test]
    async fn test_delete_scenario_5_missing_file_succeeds_cleanly() {
        let dl_temp = test_temp_dir();
        let store_temp = test_temp_dir();

        let task_id = TaskId::new();
        let filename = "non_existent_file.bin";
        let final_path = dl_temp.path().join(filename);
        let part_path = dl_temp.path().join(format!("{filename}.part"));
        assert!(!final_path.exists());
        assert!(!part_path.exists());

        let store = Arc::new(Store::open(store_temp.path()).unwrap());
        insert_test_task(
            &store,
            task_id,
            filename,
            dl_temp.path(),
            DownloadState::Failed,
        );
        let engine =
            Engine::with_store(dl_temp.path(), Arc::clone(&store), EngineConfig::default())
                .unwrap();

        let res = execute_delete_tasks(&engine, &[task_id], true).await;
        assert!(res.is_ok(), "Missing file deletion must succeed cleanly");
        assert_eq!(res.unwrap(), vec![task_id]);

        assert!(engine.get_task(task_id).unwrap().is_none());
        drop(engine);
        drop(store);
        let reopened_store = Store::open(store_temp.path()).unwrap();
        assert!(reopened_store.get_task(task_id).unwrap().is_none());
    }

    #[test]
    fn test_send_to_recycle_bin_missing_file_is_noop() {
        let non_existent = Path::new("non_existent_path_sayl_test_xyz123.bin");
        assert!(!non_existent.exists());
        let res = send_to_recycle_bin(non_existent);
        assert!(res.is_ok(), "Missing file must return Ok(())");
    }
}
