use std::path::PathBuf;
use assert_cmd::Command;
use predicates::prelude::*;
use sayl_test_http::{generate_bytes, HttpFixtureServer, Scenario};
use serde_json::Value;

struct TestDirs {
    base: PathBuf,
    dl_dir: PathBuf,
    data_dir: PathBuf,
}

impl Drop for TestDirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

fn setup_test_dirs(test_name: &str) -> TestDirs {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = PathBuf::from("target")
        .join("test_scratch")
        .join(format!("{test_name}_{nanos}"));
    let dl_dir = base.join("dl");
    let data_dir = base.join("data");
    let _ = std::fs::create_dir_all(&dl_dir);
    let _ = std::fs::create_dir_all(&data_dir);
    TestDirs {
        base,
        dl_dir,
        data_dir,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_add_prints_id_and_list_json_shows_queued() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let addr = server.start().await.unwrap();
    let url = format!("http://{}/resource.bin", addr);

    let dirs = setup_test_dirs("add_queued");

    // sayl add <url>
    let mut cmd = Command::cargo_bin("sayl").unwrap();
    let add_output = cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("add")
        .arg(&url)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let id = String::from_utf8(add_output)
        .unwrap()
        .trim()
        .lines()
        .next()
        .unwrap()
        .to_string();
    assert!(!id.is_empty(), "expected valid task id");

    // sayl list --json
    let mut list_cmd = Command::cargo_bin("sayl").unwrap();
    let list_output = list_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("--json")
        .arg("list")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let list_json: Value = serde_json::from_slice(&list_output).expect("valid JSON array");
    let array = list_json.as_array().expect("array of tasks");
    assert_eq!(array.len(), 1);
    assert_eq!(array[0]["id"].as_str().unwrap(), id);
    assert_eq!(array[0]["state"].as_str().unwrap(), "Queued");

    server.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_start_downloads_to_completion_and_bytes_match() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let addr = server.start().await.unwrap();
    let url = format!("http://{}/fixture.bin", addr);

    let dirs = setup_test_dirs("start_complete");

    // Add
    let mut add_cmd = Command::cargo_bin("sayl").unwrap();
    let add_output = add_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("add")
        .arg(&url)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let id = String::from_utf8(add_output)
        .unwrap()
        .trim()
        .lines()
        .next()
        .unwrap()
        .to_string();

    // Start
    let mut start_cmd = Command::cargo_bin("sayl").unwrap();
    start_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("start")
        .arg(&id)
        .assert()
        .success();

    // Verify downloaded file bytes
    let downloaded_file = dirs.dl_dir.join("fixture.bin");
    assert!(downloaded_file.exists(), "target file must exist");
    let bytes = std::fs::read(&downloaded_file).expect("readable downloaded file");
    let expected_size = 1048576; // 1 MiB standard fixture body
    assert_eq!(bytes.len(), expected_size);
    assert_eq!(bytes, generate_bytes(expected_size));

    server.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_start_truncated_body_exits_3_and_leaves_part() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::BodyTruncation);
    let addr = server.start().await.unwrap();
    let url = format!("http://{}/truncated.bin", addr);

    let dirs = setup_test_dirs("truncated_body");

    // Add
    let mut add_cmd = Command::cargo_bin("sayl").unwrap();
    let add_output = add_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("add")
        .arg(&url)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let id = String::from_utf8(add_output)
        .unwrap()
        .trim()
        .lines()
        .next()
        .unwrap()
        .to_string();

    // Start must fail with exit code 3
    let mut start_cmd = Command::cargo_bin("sayl").unwrap();
    start_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("start")
        .arg(&id)
        .assert()
        .code(3);

    // Verify .part file left on disk
    let part_file = dirs.dl_dir.join("truncated.bin.part");
    assert!(part_file.exists(), "part file must remain after failure");

    server.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_pause_then_start_resumes_and_completes() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let addr = server.start().await.unwrap();
    let url = format!("http://{}/resumable.bin", addr);

    let dirs = setup_test_dirs("pause_resume");

    // Add
    let mut add_cmd = Command::cargo_bin("sayl").unwrap();
    let add_output = add_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("add")
        .arg(&url)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let id = String::from_utf8(add_output)
        .unwrap()
        .trim()
        .lines()
        .next()
        .unwrap()
        .to_string();

    // Pause queued task
    let mut pause_cmd = Command::cargo_bin("sayl").unwrap();
    pause_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("pause")
        .arg(&id)
        .assert()
        .success();

    // Verify info shows Paused
    let mut info_cmd = Command::cargo_bin("sayl").unwrap();
    let info_output = info_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("--json")
        .arg("info")
        .arg(&id)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let info_json: Value = serde_json::from_slice(&info_output).unwrap();
    assert_eq!(info_json["state"].as_str().unwrap(), "Paused");

    // Resume / Start to completion
    let mut start_cmd = Command::cargo_bin("sayl").unwrap();
    start_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("start")
        .arg(&id)
        .assert()
        .success();

    let completed_file = dirs.dl_dir.join("resumable.bin");
    assert!(completed_file.exists(), "resumed file must complete");
    let bytes = std::fs::read(&completed_file).unwrap();
    assert_eq!(bytes.len(), 1048576);

    server.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_cancel_exits_4_and_second_cancel_exits_0() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let addr = server.start().await.unwrap();
    let url = format!("http://{}/cancel.bin", addr);

    let dirs = setup_test_dirs("cancel_test");

    // Add
    let mut add_cmd = Command::cargo_bin("sayl").unwrap();
    let add_output = add_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("add")
        .arg(&url)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let id = String::from_utf8(add_output)
        .unwrap()
        .trim()
        .lines()
        .next()
        .unwrap()
        .to_string();

    // First cancel must exit with code 4
    let mut cancel_1 = Command::cargo_bin("sayl").unwrap();
    cancel_1
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("cancel")
        .arg(&id)
        .assert()
        .code(4);

    // Second cancel must exit with code 0 (idempotent)
    let mut cancel_2 = Command::cargo_bin("sayl").unwrap();
    cancel_2
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("cancel")
        .arg(&id)
        .assert()
        .code(0);

    server.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_info_with_unique_prefix_and_ambiguous_prefix() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let addr = server.start().await.unwrap();
    let url1 = format!("http://{}/file1.bin", addr);
    let url2 = format!("http://{}/file2.bin", addr);

    let dirs = setup_test_dirs("info_prefix");

    // Add task 1
    let mut add_1 = Command::cargo_bin("sayl").unwrap();
    let out_1 = add_1
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("add")
        .arg(&url1)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id1 = String::from_utf8(out_1).unwrap().trim().lines().next().unwrap().to_string();

    // Add task 2
    let mut add_2 = Command::cargo_bin("sayl").unwrap();
    let out_2 = add_2
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("add")
        .arg(&url2)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id2 = String::from_utf8(out_2).unwrap().trim().lines().next().unwrap().to_string();

    let mut common_len = 0;
    let b1 = id1.as_bytes();
    let b2 = id2.as_bytes();
    while common_len < b1.len() && common_len < b2.len() && b1[common_len] == b2[common_len] {
        common_len += 1;
    }

    if common_len > 0 {
        let ambiguous_prefix = &id1[..common_len];
        let mut amb_cmd = Command::cargo_bin("sayl").unwrap();
        amb_cmd
            .arg("--download-dir")
            .arg(&dirs.dl_dir)
            .arg("--data-dir")
            .arg(&dirs.data_dir)
            .arg("info")
            .arg(ambiguous_prefix)
            .assert()
            .code(2)
            .stderr(predicate::str::contains("غامضة").or(predicate::str::contains("Ambiguous")));
    }

    // Unique prefix for task 1 (more characters than common prefix)
    let unique_len = (common_len + 1).min(id1.len());
    let unique_prefix = &id1[..unique_len];
    let mut uniq_cmd = Command::cargo_bin("sayl").unwrap();
    uniq_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("info")
        .arg(unique_prefix)
        .assert()
        .success()
        .stdout(predicate::str::contains(&id1));

    server.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_json_parses_as_valid_json() {
    let dirs = setup_test_dirs("list_json");

    let mut list_cmd = Command::cargo_bin("sayl").unwrap();
    let out = list_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("--json")
        .arg("list")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let val: Value = serde_json::from_slice(&out).expect("parsed JSON successfully");
    assert!(val.is_array());
}

#[test]
fn test_recover_on_empty_store_exits_0() {
    let dirs = setup_test_dirs("recover_empty");

    let mut cmd = Command::cargo_bin("sayl").unwrap();
    cmd.arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("recover")
        .assert()
        .success();
}

#[test]
fn test_help_lists_every_subcommand_and_documents_exit_codes() {
    let mut cmd = Command::cargo_bin("sayl").unwrap();
    let assert = cmd.arg("--help").assert().success();

    // Verify subcommands
    assert
        .stdout(predicate::str::contains("add"))
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("info"))
        .stdout(predicate::str::contains("pause"))
        .stdout(predicate::str::contains("cancel"))
        .stdout(predicate::str::contains("remove"))
        .stdout(predicate::str::contains("recover"))
        .stdout(predicate::str::contains("events"))
        // Verify exit codes documented
        .stdout(predicate::str::contains("Exit codes:"))
        .stdout(predicate::str::contains("0  Success"))
        .stdout(predicate::str::contains("2  Usage error"))
        .stdout(predicate::str::contains("3  Task failed"))
        .stdout(predicate::str::contains("4  Cancelled"))
        .stdout(predicate::str::contains("5  Store or I/O error"));
}

#[test]
fn test_global_flag_validation_connections_zero_is_usage_error() {
    let dirs = setup_test_dirs("connections_zero");

    let mut cmd = Command::cargo_bin("sayl").unwrap();
    cmd.arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("--connections")
        .arg("0")
        .arg("list")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("connections"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_start_all_downloads_multiple_queued() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let addr = server.start().await.unwrap();
    let url1 = format!("http://{}/multi1.bin", addr);
    let url2 = format!("http://{}/multi2.bin", addr);

    let dirs = setup_test_dirs("start_all");

    // Add task 1 & 2
    let mut add_cmd = Command::cargo_bin("sayl").unwrap();
    add_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("add")
        .arg(&url1)
        .arg(&url2)
        .assert()
        .success();

    // Start --all
    let mut start_cmd = Command::cargo_bin("sayl").unwrap();
    start_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("start")
        .arg("--all")
        .assert()
        .success();

    assert!(dirs.dl_dir.join("multi1.bin").exists());
    assert!(dirs.dl_dir.join("multi2.bin").exists());

    server.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_remove_deletes_task_and_files() {
    let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
    let addr = server.start().await.unwrap();
    let url = format!("http://{}/to_remove.bin", addr);

    let dirs = setup_test_dirs("remove_file");

    // Add and start
    let mut add_cmd = Command::cargo_bin("sayl").unwrap();
    let out = add_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("add")
        .arg(&url)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id = String::from_utf8(out).unwrap().trim().lines().next().unwrap().to_string();

    let mut start_cmd = Command::cargo_bin("sayl").unwrap();
    start_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("start")
        .arg(&id)
        .assert()
        .success();

    let file_path = dirs.dl_dir.join("to_remove.bin");
    assert!(file_path.exists());

    // Remove with --delete-file
    let mut remove_cmd = Command::cargo_bin("sayl").unwrap();
    remove_cmd
        .arg("--download-dir")
        .arg(&dirs.dl_dir)
        .arg("--data-dir")
        .arg(&dirs.data_dir)
        .arg("remove")
        .arg(&id)
        .arg("--delete-file")
        .assert()
        .success();

    assert!(!file_path.exists());

    server.shutdown();
}
