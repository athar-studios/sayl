//! Native messaging host installation, status reporting, extension staging, and host ping.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use sayl_bridge::install::{
    chromium_extension_dir, chromium_manifest_path, firefox_extension_dir, firefox_manifest_path,
    register_host, unregister_host, RegistryOperations, WindowsRegistry,
};

/// Registration status flags for each of the six supported browsers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredStatus {
    pub chrome: bool,
    pub edge: bool,
    pub brave: bool,
    pub opera: bool,
    pub vivaldi: bool,
    pub firefox: bool,
}

/// Overall native messaging host status matching the frontend contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeHostStatus {
    pub install_dir: String,
    pub extension_dir: String,
    pub extension_dir_firefox: String,
    pub manifest_path: String,
    pub firefox_manifest_path: String,
    pub bridge_present: bool,
    pub extension_present: bool,
    pub registered: RegisteredStatus,
}

/// Native host ping result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeHostPing {
    pub ok: bool,
    pub roundtrip_ms: u64,
    pub message: String,
}

/// Resolves the application install directory.
pub fn app_install_dir() -> PathBuf {
    if let Some(custom) = std::env::var_os("SAYL_INSTALL_DIR") {
        return PathBuf::from(custom);
    }
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            return parent.to_path_buf();
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Resolves the absolute path to `sayl-bridge.exe`.
///
/// Probes in order:
/// 1. `SAYL_BRIDGE_EXE` (direct file path or directory containing `host/sayl-bridge.exe`)
/// 2. `<install_dir>\host\sayl-bridge.exe`
/// 3. `<install_dir>\sayl-bridge.exe`
/// 4. `<install_dir>\resources\host\sayl-bridge.exe`
/// 5. `<exe_dir>\host\sayl-bridge.exe`
/// 6. `<exe_dir>\sayl-bridge.exe`
/// 7. `<exe_dir>\..\sayl-bridge.exe`
/// 8. Dev target paths (`target/debug` and `target/release`)
pub fn resolve_bridge_exe() -> Result<PathBuf, String> {
    // 1. Probe SAYL_BRIDGE_EXE env override
    if let Some(custom) = std::env::var_os("SAYL_BRIDGE_EXE") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Ok(p);
        }
        let in_host = p.join("host").join("sayl-bridge.exe");
        if in_host.is_file() {
            return Ok(in_host);
        }
        let in_root = p.join("sayl-bridge.exe");
        if in_root.is_file() {
            return Ok(in_root);
        }
    }

    let install_dir = app_install_dir();
    let has_install_dir_override = std::env::var_os("SAYL_INSTALL_DIR").is_some();

    // 2. <install_dir>\host\sayl-bridge.exe
    let cand_install_host = install_dir.join("host").join("sayl-bridge.exe");
    if cand_install_host.is_file() {
        return Ok(cand_install_host);
    }

    // 3. <install_dir>\sayl-bridge.exe
    let cand_install = install_dir.join("sayl-bridge.exe");
    if cand_install.is_file() {
        return Ok(cand_install);
    }

    // 4. <install_dir>\resources\host\sayl-bridge.exe
    let cand_install_resources = install_dir
        .join("resources")
        .join("host")
        .join("sayl-bridge.exe");
    if cand_install_resources.is_file() {
        return Ok(cand_install_resources);
    }

    // Probes 5-8: only if install_dir was not explicitly set via SAYL_INSTALL_DIR
    if !has_install_dir_override {
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(exe_dir) = current_exe.parent() {
                // 5. <exe_dir>\host\sayl-bridge.exe
                let cand_exe_host = exe_dir.join("host").join("sayl-bridge.exe");
                if cand_exe_host.is_file() {
                    return Ok(cand_exe_host);
                }

                // 6. <exe_dir>\sayl-bridge.exe
                let cand_exe = exe_dir.join("sayl-bridge.exe");
                if cand_exe.is_file() {
                    return Ok(cand_exe);
                }

                // 7. <exe_dir>\..\sayl-bridge.exe
                if let Some(parent) = exe_dir.parent() {
                    let cand_parent = parent.join("sayl-bridge.exe");
                    if cand_parent.is_file() {
                        return Ok(cand_parent);
                    }
                }
            }
        }

        // 8. Existing dev target paths
        let dev_debug =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug/sayl-bridge.exe");
        if dev_debug.is_file() {
            return Ok(dev_debug);
        }

        let dev_release =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/release/sayl-bridge.exe");
        if dev_release.is_file() {
            return Ok(dev_release);
        }
    }

    Err(format!(
        "sayl-bridge.exe not found in install dir ({}), host subdir, resources, or dev target paths",
        install_dir.display()
    ))
}

/// Locates the packaged extension source directory.
pub fn find_extension_source_dir() -> Result<PathBuf, String> {
    if let Some(custom) = std::env::var_os("SAYL_EXTENSION_SOURCE_DIR") {
        let p = PathBuf::from(custom);
        if p.exists() {
            return Ok(p);
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let candidate1 = parent.join(r"resources\extensions\browser\chromium");
            if candidate1.exists() {
                return Ok(candidate1);
            }
            let candidate2 = parent.join(r"extensions\browser\chromium");
            if candidate2.exists() {
                return Ok(candidate2);
            }
        }
    }

    let dev_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../extensions/browser/chromium");
    if dev_path.exists() {
        return Ok(dev_path);
    }

    Err("Chromium extension source directory not found".to_string())
}

/// Recursively copies all entries from `src` into `dst`.
pub fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_child = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_child)?;
        } else {
            fs::copy(entry.path(), &dest_child)?;
        }
    }
    Ok(())
}

/// Stages the browser extension files into `%LOCALAPPDATA%\Sayl\extension\...`.
pub fn stage_extension() -> Result<String, String> {
    let source_dir = find_extension_source_dir()?;
    let dest_dir = chromium_extension_dir();
    fs::create_dir_all(&dest_dir).map_err(|e| {
        format!(
            "Failed to create extension destination {}: {e}",
            dest_dir.display()
        )
    })?;

    copy_dir_all(&source_dir, &dest_dir)
        .map_err(|e| format!("Failed to stage Chromium extension: {e}"))?;

    // If Firefox extension directory exists alongside Chromium source, stage it too.
    if let Some(parent) = source_dir.parent() {
        let ff_source = parent.join("firefox");
        if ff_source.exists() && ff_source.is_dir() {
            let ff_dest = firefox_extension_dir();
            let _ = fs::create_dir_all(&ff_dest);
            let _ = copy_dir_all(&ff_source, &ff_dest);
        }
    }

    let abs_path = dest_dir.canonicalize().unwrap_or(dest_dir);
    Ok(abs_path.to_string_lossy().to_string())
}

/// Computes the native messaging host status report.
pub fn get_native_host_status<R: RegistryOperations>(registry: &R) -> NativeHostStatus {
    let install_dir = app_install_dir();
    let bridge_present = resolve_bridge_exe().is_ok();

    let ext_dir = chromium_extension_dir();
    let ext_ff_dir = firefox_extension_dir();
    let chrom_manifest = chromium_manifest_path();
    let ff_manifest = firefox_manifest_path();

    let extension_present = ext_dir.join("manifest.json").exists();

    let chrom_manifest_str = chrom_manifest.to_string_lossy().to_string();
    let ff_manifest_str = ff_manifest.to_string_lossy().to_string();

    let registered = RegisteredStatus {
        chrome: registry
            .key_has_default(sayl_bridge::install::REG_CHROME, Some(&chrom_manifest_str)),
        edge: registry.key_has_default(sayl_bridge::install::REG_EDGE, Some(&chrom_manifest_str)),
        brave: registry.key_has_default(sayl_bridge::install::REG_BRAVE, Some(&chrom_manifest_str)),
        opera: registry.key_has_default(sayl_bridge::install::REG_OPERA, Some(&chrom_manifest_str)),
        vivaldi: registry
            .key_has_default(sayl_bridge::install::REG_VIVALDI, Some(&chrom_manifest_str)),
        firefox: registry
            .key_has_default(sayl_bridge::install::REG_FIREFOX, Some(&ff_manifest_str)),
    };

    NativeHostStatus {
        install_dir: install_dir.to_string_lossy().to_string(),
        extension_dir: ext_dir.to_string_lossy().to_string(),
        extension_dir_firefox: ext_ff_dir.to_string_lossy().to_string(),
        manifest_path: chrom_manifest_str,
        firefox_manifest_path: ff_manifest_str,
        bridge_present,
        extension_present,
        registered,
    }
}

/// Registers the native host manifests and HKCU registry keys using the provided registry operations.
pub fn register_native_host_with_ops<R: RegistryOperations>(
    registry: &mut R,
    extension_id: Option<String>,
) -> Result<(), String> {
    // Stage extension first
    stage_extension()?;

    // Validate extension ID override if provided
    let valid_id = match extension_id {
        Some(ref id) => Some(sayl_bridge::install::validate_extension_id(id)?),
        None => None,
    };

    let bridge_exe =
        resolve_bridge_exe().map_err(|e| format!("Cannot register native host: {e}"))?;
    let abs_bridge_exe = if bridge_exe.is_absolute() {
        bridge_exe
    } else {
        std::env::current_dir()
            .map_err(|e| format!("Failed to get current dir: {e}"))?
            .join(bridge_exe)
    };

    if !abs_bridge_exe.exists() {
        return Err(format!(
            "Cannot register native host: bridge executable does not exist at {}",
            abs_bridge_exe.display()
        ));
    }

    register_host(registry, &abs_bridge_exe, valid_id.as_deref())?;
    Ok(())
}

/// Unregisters the native host by deleting manifests and HKCU registry keys.
pub fn unregister_native_host_with_ops<R: RegistryOperations>(
    registry: &mut R,
) -> Result<(), String> {
    unregister_host(registry)
}

/// Evaluates raw ping output into a structured [`NativeHostPing`] result.
pub fn evaluate_ping_result(
    roundtrip_ms: u64,
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
    timed_out: bool,
) -> NativeHostPing {
    if timed_out {
        return NativeHostPing {
            ok: false,
            roundtrip_ms,
            message: "Ping timed out after 3s".to_string(),
        };
    }

    match exit_code {
        Some(0) => {
            if stdout.contains("Pong") {
                NativeHostPing {
                    ok: true,
                    roundtrip_ms,
                    message: stdout.to_string(),
                }
            } else {
                NativeHostPing {
                    ok: false,
                    roundtrip_ms,
                    message: format!("Unexpected ping output: {stdout}"),
                }
            }
        }
        Some(code) => {
            let msg = if !stderr.is_empty() {
                stderr.to_string()
            } else if !stdout.is_empty() {
                stdout.to_string()
            } else {
                format!("sayl-bridge exited with code {code}")
            };
            NativeHostPing {
                ok: false,
                roundtrip_ms,
                message: msg,
            }
        }
        None => NativeHostPing {
            ok: false,
            roundtrip_ms,
            message: "sayl-bridge terminated unexpectedly".to_string(),
        },
    }
}

/// Asynchronously spawns `sayl-bridge.exe --ping` with a 3s timeout.
pub async fn run_bridge_ping(bridge_exe: &Path) -> NativeHostPing {
    if !bridge_exe.exists() {
        return NativeHostPing {
            ok: false,
            roundtrip_ms: 0,
            message: format!("sayl-bridge.exe not found at {}", bridge_exe.display()),
        };
    }

    let start = std::time::Instant::now();
    let mut cmd = tokio::process::Command::new(bridge_exe);
    cmd.arg("--ping");
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(0x08000000);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let timeout_duration = std::time::Duration::from_millis(3000);
    match tokio::time::timeout(timeout_duration, cmd.output()).await {
        Ok(Ok(output)) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();
            evaluate_ping_result(
                elapsed,
                output.status.code(),
                &stdout_str,
                &stderr_str,
                false,
            )
        }
        Ok(Err(err)) => {
            let elapsed = start.elapsed().as_millis() as u64;
            NativeHostPing {
                ok: false,
                roundtrip_ms: elapsed,
                message: format!("Failed to spawn bridge process: {err}"),
            }
        }
        Err(_) => NativeHostPing {
            ok: false,
            roundtrip_ms: 3000,
            message: "Ping timed out after 3s".to_string(),
        },
    }
}

/// Opens Windows Explorer focused on the staged Chromium extension directory.
pub fn open_extension_folder() -> Result<(), String> {
    let dir = chromium_extension_dir();
    let _ = fs::create_dir_all(&dir);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("explorer");
        cmd.arg(&dir);
        cmd.creation_flags(0x08000000);
        cmd.spawn()
            .map_err(|e| format!("Failed to open extension directory in explorer: {e}"))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = dir;
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// Tauri Command Implementations
// ----------------------------------------------------------------------------

pub fn cmd_native_host_status() -> NativeHostStatus {
    let reg = WindowsRegistry;
    get_native_host_status(&reg)
}

pub async fn cmd_browser_status() -> Vec<crate::browsers::BrowserStatus> {
    let reg = WindowsRegistry;
    let probe = crate::browsers::SystemBrowserProbe;

    let ping_ok = if let Ok(bridge_exe) = resolve_bridge_exe() {
        run_bridge_ping(&bridge_exe).await.ok
    } else {
        false
    };

    crate::browsers::collect_browser_statuses(&probe, &reg, ping_ok)
}

pub fn cmd_prepare_extension() -> Result<String, String> {
    stage_extension()
}

pub fn cmd_register_native_host(extension_id: Option<String>) -> Result<(), String> {
    let mut reg = WindowsRegistry;
    register_native_host_with_ops(&mut reg, extension_id)
}

pub fn cmd_unregister_native_host() -> Result<(), String> {
    let mut reg = WindowsRegistry;
    unregister_native_host_with_ops(&mut reg)
}

pub async fn cmd_test_native_host() -> Result<NativeHostPing, String> {
    let bridge_exe = match resolve_bridge_exe() {
        Ok(p) => p,
        Err(e) => {
            return Ok(NativeHostPing {
                ok: false,
                roundtrip_ms: 0,
                message: e,
            });
        }
    };
    Ok(run_bridge_ping(&bridge_exe).await)
}

pub fn cmd_open_extension_folder() -> Result<(), String> {
    open_extension_folder()
}

pub fn cmd_open_browser_extensions(browser_id: String) -> Result<(), String> {
    let probe = crate::browsers::SystemBrowserProbe;
    crate::browsers::launch_browser_extensions_page(&probe, &browser_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sayl_bridge::install::MockRegistry;
    use std::sync::Mutex;

    static TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

    fn make_test_temp_dir() -> PathBuf {
        let unique_name = format!(
            "test-tauri-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let target_tmp = Path::new("target").join("test-tmp").join(unique_name);
        let _ = std::fs::create_dir_all(&target_tmp);
        target_tmp
    }

    #[test]
    fn test_status_serialization_shape() {
        let status = NativeHostStatus {
            install_dir: r"C:\Program Files\Sayl".to_string(),
            extension_dir: r"C:\Users\User\AppData\Local\Sayl\extension\chromium".to_string(),
            extension_dir_firefox: r"C:\Users\User\AppData\Local\Sayl\extension\firefox"
                .to_string(),
            manifest_path: r"C:\Users\User\AppData\Local\Sayl\host\com.sayl.downloader.json"
                .to_string(),
            firefox_manifest_path:
                r"C:\Users\User\AppData\Local\Sayl\host\com.sayl.downloader.firefox.json"
                    .to_string(),
            bridge_present: true,
            extension_present: true,
            registered: RegisteredStatus {
                chrome: true,
                edge: true,
                brave: false,
                opera: false,
                vivaldi: false,
                firefox: true,
            },
        };

        let json_val = serde_json::to_value(&status).expect("serializes to json");
        let obj = json_val.as_object().expect("json object");

        assert!(obj.contains_key("install_dir"));
        assert!(obj.contains_key("extension_dir"));
        assert!(obj.contains_key("extension_dir_firefox"));
        assert!(obj.contains_key("manifest_path"));
        assert!(obj.contains_key("firefox_manifest_path"));
        assert!(obj.contains_key("bridge_present"));
        assert!(obj.contains_key("extension_present"));
        assert!(obj.contains_key("registered"));

        let reg_obj = obj["registered"].as_object().expect("registered object");
        assert_eq!(reg_obj["chrome"], true);
        assert_eq!(reg_obj["edge"], true);
        assert_eq!(reg_obj["brave"], false);
        assert_eq!(reg_obj["opera"], false);
        assert_eq!(reg_obj["vivaldi"], false);
        assert_eq!(reg_obj["firefox"], true);
    }

    #[test]
    fn test_ping_result_handling() {
        // 1. Success case
        let success = evaluate_ping_result(
            42,
            Some(0),
            r#"{"type":"Pong","version":1,"app_version":"0.1.0"}"#,
            "",
            false,
        );
        assert!(success.ok);
        assert_eq!(success.roundtrip_ms, 42);
        assert!(success.message.contains("Pong"));

        // 2. Non-zero exit code
        let non_zero =
            evaluate_ping_result(15, Some(1), "", "Init failed: permission denied", false);
        assert!(!non_zero.ok);
        assert_eq!(non_zero.roundtrip_ms, 15);
        assert!(non_zero.message.contains("Init failed"));

        // 3. Timeout
        let timed_out = evaluate_ping_result(3000, None, "", "", true);
        assert!(!timed_out.ok);
        assert_eq!(timed_out.roundtrip_ms, 3000);
        assert!(timed_out.message.contains("timed out"));
    }

    #[test]
    fn test_prepare_extension_staging_and_idempotency() {
        let _lock = TEST_ENV_LOCK.lock().unwrap();
        let temp_base = make_test_temp_dir();
        let local_app_data = temp_base.join("localappdata");
        let fixture_src = temp_base.join("fixture_extension");

        fs::create_dir_all(&local_app_data).expect("create localappdata");
        fs::create_dir_all(&fixture_src).expect("create fixture src");

        // Write sample manifest.json and background.js
        fs::write(
            fixture_src.join("manifest.json"),
            br#"{"manifest_version": 3, "name": "Test"}"#,
        )
        .expect("write manifest");
        fs::write(fixture_src.join("background.js"), b"console.log('hello');")
            .expect("write background.js");

        std::env::set_var("SAYL_LOCALAPPDATA", &local_app_data);
        std::env::set_var("SAYL_EXTENSION_SOURCE_DIR", &fixture_src);

        // First staging run
        let staged_path_str = stage_extension().expect("stages extension first time");
        let staged_path = PathBuf::from(staged_path_str);
        assert!(staged_path.exists());
        assert!(staged_path.join("manifest.json").exists());
        assert!(staged_path.join("background.js").exists());

        // Second staging run (idempotency check)
        let staged_path_2 = stage_extension().expect("stages extension second time idempotently");
        assert_eq!(staged_path.to_string_lossy(), staged_path_2);
        assert!(staged_path.join("manifest.json").exists());

        std::env::remove_var("SAYL_LOCALAPPDATA");
        std::env::remove_var("SAYL_EXTENSION_SOURCE_DIR");
    }

    #[test]
    fn test_native_host_registration_flow_with_mock() {
        let _lock = TEST_ENV_LOCK.lock().unwrap();
        let temp_base = make_test_temp_dir();
        let local_app_data = temp_base.join("localappdata");
        let fixture_src = temp_base.join("fixture_extension");
        let bridge_exe = temp_base.join("sayl-bridge.exe");

        fs::create_dir_all(&local_app_data).unwrap();
        fs::create_dir_all(&fixture_src).unwrap();
        fs::write(fixture_src.join("manifest.json"), b"{}").unwrap();
        fs::write(&bridge_exe, b"dummy bridge binary").unwrap();

        std::env::set_var("SAYL_LOCALAPPDATA", &local_app_data);
        std::env::set_var("SAYL_EXTENSION_SOURCE_DIR", &fixture_src);
        std::env::set_var("SAYL_BRIDGE_EXE", &bridge_exe);

        let mut mock_reg = MockRegistry::default();

        // Register host
        register_native_host_with_ops(&mut mock_reg, None).expect("registers native host");

        // Status report check
        let status = get_native_host_status(&mock_reg);
        assert!(status.bridge_present);
        assert!(status.extension_present);
        assert!(status.registered.chrome);
        assert!(status.registered.edge);
        assert!(status.registered.brave);
        assert!(status.registered.opera);
        assert!(status.registered.vivaldi);
        assert!(status.registered.firefox);

        // Unregister
        unregister_native_host_with_ops(&mut mock_reg).expect("unregisters native host");
        let status_after = get_native_host_status(&mock_reg);
        assert!(!status_after.registered.chrome);
        assert!(!status_after.registered.edge);
        assert!(!status_after.registered.firefox);

        std::env::remove_var("SAYL_LOCALAPPDATA");
        std::env::remove_var("SAYL_EXTENSION_SOURCE_DIR");
        std::env::remove_var("SAYL_BRIDGE_EXE");
    }

    #[test]
    fn test_resolve_bridge_exe_in_installed_host_subdir() {
        let _lock = TEST_ENV_LOCK.lock().unwrap();
        let temp_base = make_test_temp_dir();
        let install_dir = temp_base.join("install");
        let host_dir = install_dir.join("host");
        fs::create_dir_all(&host_dir).unwrap();
        let bridge_exe = host_dir.join("sayl-bridge.exe");
        fs::write(&bridge_exe, b"dummy bridge binary in host subdir").unwrap();

        // 1. Via SAYL_INSTALL_DIR
        std::env::set_var("SAYL_INSTALL_DIR", &install_dir);
        std::env::remove_var("SAYL_BRIDGE_EXE");
        let resolved = resolve_bridge_exe().expect("resolves bridge in host subdir");
        assert_eq!(resolved, bridge_exe);
        std::env::remove_var("SAYL_INSTALL_DIR");

        // 2. Via SAYL_BRIDGE_EXE pointing to temp dir
        std::env::set_var("SAYL_BRIDGE_EXE", &install_dir);
        let resolved_via_env =
            resolve_bridge_exe().expect("resolves bridge via SAYL_BRIDGE_EXE dir");
        assert_eq!(resolved_via_env, bridge_exe);
        std::env::remove_var("SAYL_BRIDGE_EXE");
    }

    #[test]
    fn test_register_native_host_missing_bridge_returns_error() {
        let _lock = TEST_ENV_LOCK.lock().unwrap();
        let temp_base = make_test_temp_dir();
        let local_app_data = temp_base.join("localappdata");
        let fixture_src = temp_base.join("fixture_extension");
        let empty_install = temp_base.join("empty_install");

        fs::create_dir_all(&local_app_data).unwrap();
        fs::create_dir_all(&fixture_src).unwrap();
        fs::create_dir_all(&empty_install).unwrap();
        fs::write(fixture_src.join("manifest.json"), b"{}").unwrap();

        std::env::set_var("SAYL_LOCALAPPDATA", &local_app_data);
        std::env::set_var("SAYL_EXTENSION_SOURCE_DIR", &fixture_src);
        std::env::set_var("SAYL_INSTALL_DIR", &empty_install);
        std::env::remove_var("SAYL_BRIDGE_EXE");

        let mut mock_reg = MockRegistry::default();
        let err = register_native_host_with_ops(&mut mock_reg, None).unwrap_err();
        assert!(
            err.contains("sayl-bridge.exe not found")
                || err.contains("Cannot register native host"),
            "Expected missing bridge error, got: {err}"
        );

        let status = get_native_host_status(&mock_reg);
        assert!(!status.bridge_present);

        std::env::remove_var("SAYL_LOCALAPPDATA");
        std::env::remove_var("SAYL_EXTENSION_SOURCE_DIR");
        std::env::remove_var("SAYL_INSTALL_DIR");
    }
}
