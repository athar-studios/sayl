//! Native messaging host installation, manifests, and registry management.
//!
//! Provides CLI handlers and helpers to register/unregister the SAYL native
//! messaging host for Chromium-based browsers (Chrome, Edge, Brave, Opera, Vivaldi)
//! and Firefox under HKCU without requiring administrator elevation.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::host::{CommandLine, Dispatcher, EchoDispatcher};
use crate::message::{Request, Response, PROTOCOL_VERSION};
use crate::protocol::{decode_frame, write_message};

/// The host name used by browsers to locate the native messaging host.
pub const HOST_NAME: &str = "com.sayl.downloader";

/// Human-readable host description in the manifest.
pub const HOST_DESCRIPTION: &str = "SAYL native messaging host";

/// The pinned Chromium extension ID whose public key is pinned in the repository.
pub const PINNED_CHROMIUM_EXTENSION_ID: &str = "cpigidmkbhcfkgljofmmpnfdhdaoldoj";

/// The Firefox add-on ID.
pub const FIREFOX_EXTENSION_ID: &str = "sayl@atharstudios.com";

// Registry key paths under HKCU for each supported browser.
pub const REG_CHROME: &str =
    r"HKCU\Software\Google\Chrome\NativeMessagingHosts\com.sayl.downloader";
pub const REG_EDGE: &str = r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.sayl.downloader";
pub const REG_BRAVE: &str =
    r"HKCU\Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.sayl.downloader";
pub const REG_OPERA: &str =
    r"HKCU\Software\Opera Software\Opera Stable\NativeMessagingHosts\com.sayl.downloader";
pub const REG_VIVALDI: &str = r"HKCU\Software\Vivaldi\NativeMessagingHosts\com.sayl.downloader";
pub const REG_FIREFOX: &str = r"HKCU\Software\Mozilla\NativeMessagingHosts\com.sayl.downloader";

/// All six registry keys with their corresponding browser identifier.
pub const ALL_REGISTRY_KEYS: &[(&str, &str)] = &[
    ("chrome", REG_CHROME),
    ("edge", REG_EDGE),
    ("brave", REG_BRAVE),
    ("opera", REG_OPERA),
    ("vivaldi", REG_VIVALDI),
    ("firefox", REG_FIREFOX),
];

/// Returns the `%LOCALAPPDATA%` root directory.
///
/// If `SAYL_LOCALAPPDATA` is set (used for tests), it overrides `%LOCALAPPDATA%`.
/// Production code must not set or rely on this override.
pub fn local_app_data_root() -> PathBuf {
    if let Some(custom) = std::env::var_os("SAYL_LOCALAPPDATA") {
        return PathBuf::from(custom);
    }
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local_app_data);
    }
    if let Some(userprofile) = std::env::var_os("USERPROFILE") {
        return PathBuf::from(userprofile).join("AppData").join("Local");
    }
    std::env::temp_dir()
}

/// Base directory for SAYL data in LocalAppData (`%LOCALAPPDATA%\Sayl`).
pub fn sayl_local_data_dir() -> PathBuf {
    local_app_data_root().join("Sayl")
}

/// Directory where native messaging host manifests are stored (`%LOCALAPPDATA%\Sayl\host`).
pub fn host_dir() -> PathBuf {
    sayl_local_data_dir().join("host")
}

/// Path to the Chromium native messaging manifest (`%LOCALAPPDATA%\Sayl\host\com.sayl.downloader.json`).
pub fn chromium_manifest_path() -> PathBuf {
    host_dir().join(format!("{HOST_NAME}.json"))
}

/// Path to the Firefox native messaging manifest (`%LOCALAPPDATA%\Sayl\host\com.sayl.downloader.firefox.json`).
pub fn firefox_manifest_path() -> PathBuf {
    host_dir().join(format!("{HOST_NAME}.firefox.json"))
}

/// Directory for staging the unpacked Chromium extension (`%LOCALAPPDATA%\Sayl\extension\chromium`).
pub fn chromium_extension_dir() -> PathBuf {
    sayl_local_data_dir().join("extension").join("chromium")
}

/// Directory for staging the Firefox extension (`%LOCALAPPDATA%\Sayl\extension\firefox`).
pub fn firefox_extension_dir() -> PathBuf {
    sayl_local_data_dir().join("extension").join("firefox")
}

/// Validates an extension ID.
///
/// Returns `Ok(id)` if the ID matches the pinned ID or is a valid 32-character
/// lowercase string containing only `[a-p]`. Returns an error otherwise.
pub fn validate_extension_id(id: &str) -> Result<String, String> {
    let trimmed = id.trim();
    if trimmed == PINNED_CHROMIUM_EXTENSION_ID {
        return Ok(trimmed.to_string());
    }
    if trimmed.len() == 32
        && trimmed
            .chars()
            .all(|c| c.is_ascii_lowercase() && ('a'..='p').contains(&c))
    {
        return Ok(trimmed.to_string());
    }
    Err(format!(
        "Invalid extension ID: '{id}'. Must match pinned ID '{PINNED_CHROMIUM_EXTENSION_ID}' or be 32 lowercase chars in [a-p]."
    ))
}

/// Chromium Native Messaging Host manifest structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChromiumManifest {
    pub name: String,
    pub description: String,
    pub path: String,
    #[serde(rename = "type")]
    pub host_type: String,
    pub allowed_origins: Vec<String>,
}

/// Firefox Native Messaging Host manifest structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirefoxManifest {
    pub name: String,
    pub description: String,
    pub path: String,
    #[serde(rename = "type")]
    pub host_type: String,
    pub allowed_extensions: Vec<String>,
}

/// Constructs a Chromium manifest with validated absolute bridge executable path.
pub fn build_chromium_manifest(
    exe_path: &Path,
    extension_id: Option<&str>,
) -> Result<ChromiumManifest, String> {
    if !exe_path.is_absolute() {
        return Err(format!(
            "Bridge executable path must be absolute: {}",
            exe_path.display()
        ));
    }
    let valid_id = match extension_id {
        Some(id) => validate_extension_id(id)?,
        None => PINNED_CHROMIUM_EXTENSION_ID.to_string(),
    };

    Ok(ChromiumManifest {
        name: HOST_NAME.to_string(),
        description: HOST_DESCRIPTION.to_string(),
        path: exe_path.to_string_lossy().to_string(),
        host_type: "stdio".to_string(),
        allowed_origins: vec![format!("chrome-extension://{valid_id}/")],
    })
}

/// Constructs a Firefox manifest with validated absolute bridge executable path.
pub fn build_firefox_manifest(exe_path: &Path) -> Result<FirefoxManifest, String> {
    if !exe_path.is_absolute() {
        return Err(format!(
            "Bridge executable path must be absolute: {}",
            exe_path.display()
        ));
    }

    Ok(FirefoxManifest {
        name: HOST_NAME.to_string(),
        description: HOST_DESCRIPTION.to_string(),
        path: exe_path.to_string_lossy().to_string(),
        host_type: "stdio".to_string(),
        allowed_extensions: vec![FIREFOX_EXTENSION_ID.to_string()],
    })
}

/// Abstraction for Windows registry operations to allow safe, side-effect-free testing.
pub trait RegistryOperations {
    /// Sets the default value of the specified registry key.
    fn set_key_default(&mut self, key_path: &str, value: &str) -> Result<(), String>;
    /// Deletes the specified registry key.
    fn delete_key(&mut self, key_path: &str) -> Result<(), String>;
    /// Checks whether the specified registry key exists.
    fn key_exists(&self, key_path: &str) -> bool;
    /// Checks whether the key's default value matches an expected string if provided.
    fn key_has_default(&self, key_path: &str, expected_value: Option<&str>) -> bool;
}

#[cfg(target_os = "windows")]
fn parse_reg_key(full_path: &str) -> Option<(winreg::RegKey, &str)> {
    if let Some(sub) = full_path.strip_prefix(r"HKCU\") {
        Some((
            winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER),
            sub,
        ))
    } else if let Some(sub) = full_path.strip_prefix(r"HKEY_CURRENT_USER\") {
        Some((
            winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER),
            sub,
        ))
    } else if let Some(sub) = full_path.strip_prefix(r"HKLM\") {
        Some((
            winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE),
            sub,
        ))
    } else if let Some(sub) = full_path.strip_prefix(r"HKEY_LOCAL_MACHINE\") {
        Some((
            winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE),
            sub,
        ))
    } else {
        None
    }
}

/// Production implementation of [`RegistryOperations`] using Windows `winreg`.
pub struct WindowsRegistry;

impl RegistryOperations for WindowsRegistry {
    fn set_key_default(&mut self, key_path: &str, value: &str) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            let (root, subkey) = parse_reg_key(key_path)
                .ok_or_else(|| format!("Invalid registry root in {key_path}"))?;
            let (key, _) = root
                .create_subkey(subkey)
                .map_err(|e| format!("Failed to create/open registry key {key_path}: {e}"))?;
            key.set_value("", &value)
                .map_err(|e| format!("Failed to set default value for {key_path}: {e}"))?;
            Ok(())
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (key_path, value);
            Ok(())
        }
    }

    fn delete_key(&mut self, key_path: &str) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            if let Some((root, subkey)) = parse_reg_key(key_path) {
                match root.delete_subkey_all(subkey) {
                    Ok(()) => Ok(()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(e) => Err(format!("Failed to delete registry key {key_path}: {e}")),
                }
            } else {
                Ok(())
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = key_path;
            Ok(())
        }
    }

    fn key_exists(&self, key_path: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            if let Some((root, subkey)) = parse_reg_key(key_path) {
                root.open_subkey(subkey).is_ok()
            } else {
                false
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = key_path;
            false
        }
    }

    fn key_has_default(&self, key_path: &str, expected_value: Option<&str>) -> bool {
        #[cfg(target_os = "windows")]
        {
            let (root, subkey) = match parse_reg_key(key_path) {
                Some(pair) => pair,
                None => return false,
            };
            let key = match root.open_subkey(subkey) {
                Ok(k) => k,
                Err(_) => return false,
            };
            if let Some(expected) = expected_value {
                match key.get_value::<String, _>("") {
                    Ok(val) => val == expected,
                    Err(_) => false,
                }
            } else {
                true
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (key_path, expected_value);
            false
        }
    }
}

/// In-memory mock implementation of [`RegistryOperations`] for unit tests.
#[derive(Debug, Default, Clone)]
pub struct MockRegistry {
    pub keys: std::collections::HashMap<String, String>,
}

impl RegistryOperations for MockRegistry {
    fn set_key_default(&mut self, key_path: &str, value: &str) -> Result<(), String> {
        self.keys.insert(key_path.to_string(), value.to_string());
        Ok(())
    }

    fn delete_key(&mut self, key_path: &str) -> Result<(), String> {
        self.keys.remove(key_path);
        Ok(())
    }

    fn key_exists(&self, key_path: &str) -> bool {
        self.keys.contains_key(key_path)
    }

    fn key_has_default(&self, key_path: &str, expected_value: Option<&str>) -> bool {
        match self.keys.get(key_path) {
            Some(actual) => {
                if let Some(expected) = expected_value {
                    actual == expected
                } else {
                    true
                }
            }
            None => false,
        }
    }
}

/// JSON summary returned by `--register-host`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistrationSummary {
    pub status: String,
    pub host_name: String,
    pub bridge_path: String,
    pub extension_id: String,
    pub chromium_manifest: String,
    pub firefox_manifest: String,
    pub registered_keys: Vec<String>,
}

/// Registers the native messaging host manifests and registry keys.
pub fn register_host<R: RegistryOperations>(
    registry: &mut R,
    bridge_exe_path: &Path,
    extension_id: Option<&str>,
) -> Result<RegistrationSummary, String> {
    let abs_bridge_path = if bridge_exe_path.is_absolute() {
        bridge_exe_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("Failed to get current dir: {e}"))?
            .join(bridge_exe_path)
    };

    let host_directory = host_dir();
    fs::create_dir_all(&host_directory).map_err(|e| {
        format!(
            "Failed to create host directory {}: {e}",
            host_directory.display()
        )
    })?;

    let chrom_manifest = build_chromium_manifest(&abs_bridge_path, extension_id)?;
    let chrom_path = chromium_manifest_path();
    let chrom_json = serde_json::to_string_pretty(&chrom_manifest)
        .map_err(|e| format!("Failed to serialize Chromium manifest: {e}"))?;
    fs::write(&chrom_path, chrom_json)
        .map_err(|e| format!("Failed to write Chromium manifest: {e}"))?;

    let ff_manifest = build_firefox_manifest(&abs_bridge_path)?;
    let ff_path = firefox_manifest_path();
    let ff_json = serde_json::to_string_pretty(&ff_manifest)
        .map_err(|e| format!("Failed to serialize Firefox manifest: {e}"))?;
    fs::write(&ff_path, ff_json).map_err(|e| format!("Failed to write Firefox manifest: {e}"))?;

    let chrom_manifest_str = chrom_path.to_string_lossy().to_string();
    let ff_manifest_str = ff_path.to_string_lossy().to_string();

    let mut registered_keys = Vec::new();
    for (browser_id, key_path) in ALL_REGISTRY_KEYS {
        let manifest_target = if *browser_id == "firefox" {
            &ff_manifest_str
        } else {
            &chrom_manifest_str
        };
        registry.set_key_default(key_path, manifest_target)?;
        registered_keys.push((*key_path).to_string());
    }

    let actual_ext_id = extension_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| PINNED_CHROMIUM_EXTENSION_ID.to_string());

    Ok(RegistrationSummary {
        status: "ok".to_string(),
        host_name: HOST_NAME.to_string(),
        bridge_path: abs_bridge_path.to_string_lossy().to_string(),
        extension_id: actual_ext_id,
        chromium_manifest: chrom_manifest_str,
        firefox_manifest: ff_manifest_str,
        registered_keys,
    })
}

/// Unregisters the native messaging host by deleting registry keys and manifests.
pub fn unregister_host<R: RegistryOperations>(registry: &mut R) -> Result<(), String> {
    for (_browser_id, key_path) in ALL_REGISTRY_KEYS {
        let _ = registry.delete_key(key_path);
    }

    let chrom_path = chromium_manifest_path();
    if chrom_path.exists() {
        let _ = fs::remove_file(&chrom_path);
    }

    let ff_path = firefox_manifest_path();
    if ff_path.exists() {
        let _ = fs::remove_file(&ff_path);
    }

    Ok(())
}

/// CLI modes supported by `sayl-bridge.exe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeCliMode {
    /// Normal native messaging stdio loop.
    Serve(CommandLine),
    /// Register host manifests and registry keys.
    RegisterHost { extension_id: Option<String> },
    /// Unregister host registry keys and remove manifests.
    UnregisterHost,
    /// Ping dispatcher test and print Pong JSON.
    Ping,
}

/// Parses CLI arguments to determine binary run mode.
pub fn parse_cli_args<I, S>(args: I) -> Result<BridgeCliMode, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    if args.is_empty() {
        return Ok(BridgeCliMode::Serve(CommandLine::default()));
    }

    let mut is_register = false;
    let mut is_unregister = false;
    let mut is_ping = false;
    let mut extension_id = None;

    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "--register-host" {
            is_register = true;
        } else if arg == "--unregister-host" {
            is_unregister = true;
        } else if arg == "--ping" {
            is_ping = true;
        } else if arg == "--extension-id" {
            if let Some(val) = iter.next() {
                extension_id = Some(validate_extension_id(val)?);
            } else {
                return Err("Missing value for --extension-id".to_string());
            }
        } else if let Some(val) = arg.strip_prefix("--extension-id=") {
            extension_id = Some(validate_extension_id(val)?);
        }
    }

    if is_register {
        return Ok(BridgeCliMode::RegisterHost { extension_id });
    }
    if is_unregister {
        return Ok(BridgeCliMode::UnregisterHost);
    }
    if is_ping {
        return Ok(BridgeCliMode::Ping);
    }

    // Pass through to serve mode with original arguments (origins, window handles)
    Ok(BridgeCliMode::Serve(CommandLine::from_args(args)))
}

/// Executes a ping request through the framing and dispatcher layer.
pub fn execute_ping() -> Result<Response, String> {
    let req = Request::Ping {
        version: PROTOCOL_VERSION,
    };
    let mut frame = Vec::new();
    write_message(&mut frame, &req).map_err(|e| format!("Failed to encode ping: {e}"))?;

    let mut cursor = std::io::Cursor::new(frame);
    let payload = decode_frame(&mut cursor)
        .map_err(|e| format!("Failed to decode ping frame: {e}"))?
        .ok_or_else(|| "Ping frame was empty".to_string())?;

    let req = serde_json::from_slice::<Request>(&payload)
        .map_err(|e| format!("Failed to parse ping request: {e}"))?;

    let mut dispatcher = EchoDispatcher;
    let resp = dispatcher.handle(req);
    match resp {
        Response::Pong { .. } => Ok(resp),
        Response::Error { code, message } => {
            Err(format!("Dispatcher returned error {code}: {message}"))
        }
        _ => Err("Dispatcher returned unexpected response".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chromium_manifest_content_and_keys() {
        let exe = PathBuf::from(r"C:\Program Files\Sayl\sayl-bridge.exe");
        let manifest = build_chromium_manifest(&exe, None).expect("builds chromium manifest");
        assert_eq!(manifest.name, "com.sayl.downloader");
        assert_eq!(manifest.description, "SAYL native messaging host");
        assert_eq!(manifest.path, r"C:\Program Files\Sayl\sayl-bridge.exe");
        assert_eq!(manifest.host_type, "stdio");
        assert_eq!(
            manifest.allowed_origins,
            vec!["chrome-extension://cpigidmkbhcfkgljofmmpnfdhdaoldoj/"]
        );

        let json = serde_json::to_value(&manifest).expect("serializes to json");
        assert_eq!(json["name"], "com.sayl.downloader");
        assert_eq!(json["type"], "stdio");
        assert_eq!(
            json["allowed_origins"][0],
            "chrome-extension://cpigidmkbhcfkgljofmmpnfdhdaoldoj/"
        );
        // Ensure Firefox key allowed_extensions is absent
        assert!(json.get("allowed_extensions").is_none());
    }

    #[test]
    fn test_firefox_manifest_content_and_keys() {
        let exe = PathBuf::from(r"C:\Program Files\Sayl\sayl-bridge.exe");
        let manifest = build_firefox_manifest(&exe).expect("builds firefox manifest");
        assert_eq!(manifest.name, "com.sayl.downloader");
        assert_eq!(manifest.description, "SAYL native messaging host");
        assert_eq!(manifest.path, r"C:\Program Files\Sayl\sayl-bridge.exe");
        assert_eq!(manifest.host_type, "stdio");
        assert_eq!(manifest.allowed_extensions, vec!["sayl@atharstudios.com"]);

        let json = serde_json::to_value(&manifest).expect("serializes to json");
        assert_eq!(json["name"], "com.sayl.downloader");
        assert_eq!(json["type"], "stdio");
        assert_eq!(json["allowed_extensions"][0], "sayl@atharstudios.com");
        // Ensure Chromium key allowed_origins is absent
        assert!(json.get("allowed_origins").is_none());
    }

    #[test]
    fn test_manifest_rejects_relative_path() {
        let rel_exe = Path::new("sayl-bridge.exe");
        assert!(build_chromium_manifest(rel_exe, None).is_err());
        assert!(build_firefox_manifest(rel_exe).is_err());
    }

    #[test]
    fn test_extension_id_validation() {
        // Pinned ID is valid
        assert!(validate_extension_id(PINNED_CHROMIUM_EXTENSION_ID).is_ok());

        // Valid 32-char [a-p] string
        let valid_custom = "abcdefghijklmnopabcdefghijklmnop";
        assert_eq!(validate_extension_id(valid_custom).unwrap(), valid_custom);

        // Invalid: contains chars outside [a-p]
        let invalid_char = "abcdefghijklmnopabcdefghijklmnoz";
        assert!(validate_extension_id(invalid_char).is_err());

        // Invalid length
        let too_short = "abcdef";
        assert!(validate_extension_id(too_short).is_err());

        // Arbitrary URL / origin injection attempt
        let evil_url = "https://evil.com";
        assert!(validate_extension_id(evil_url).is_err());
    }

    #[test]
    fn test_registry_key_path_constants() {
        assert_eq!(
            REG_CHROME,
            r"HKCU\Software\Google\Chrome\NativeMessagingHosts\com.sayl.downloader"
        );
        assert_eq!(
            REG_EDGE,
            r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.sayl.downloader"
        );
        assert_eq!(
            REG_BRAVE,
            r"HKCU\Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.sayl.downloader"
        );
        assert_eq!(
            REG_OPERA,
            r"HKCU\Software\Opera Software\Opera Stable\NativeMessagingHosts\com.sayl.downloader"
        );
        assert_eq!(
            REG_VIVALDI,
            r"HKCU\Software\Vivaldi\NativeMessagingHosts\com.sayl.downloader"
        );
        assert_eq!(
            REG_FIREFOX,
            r"HKCU\Software\Mozilla\NativeMessagingHosts\com.sayl.downloader"
        );
        assert_eq!(ALL_REGISTRY_KEYS.len(), 6);
    }

    #[test]
    fn test_cli_argument_parsing() {
        // No flags -> Serve
        let empty: Vec<String> = vec![];
        assert!(matches!(
            parse_cli_args(empty).unwrap(),
            BridgeCliMode::Serve(_)
        ));

        // Browser origin arg -> Serve
        let browser_args = vec![
            "chrome-extension://cpigidmkbhcfkgljofmmpnfdhdaoldoj/".to_string(),
            "--parent-window=123".to_string(),
        ];
        match parse_cli_args(browser_args).unwrap() {
            BridgeCliMode::Serve(cmd) => {
                assert_eq!(
                    cmd.caller_origin.as_deref(),
                    Some("chrome-extension://cpigidmkbhcfkgljofmmpnfdhdaoldoj/")
                );
            }
            _ => panic!("Expected Serve mode"),
        }

        // Register host default
        let reg = vec!["--register-host".to_string()];
        assert_eq!(
            parse_cli_args(reg).unwrap(),
            BridgeCliMode::RegisterHost { extension_id: None }
        );

        // Register host with extension id space separated
        let reg_with_id = vec![
            "--register-host".to_string(),
            "--extension-id".to_string(),
            PINNED_CHROMIUM_EXTENSION_ID.to_string(),
        ];
        assert_eq!(
            parse_cli_args(reg_with_id).unwrap(),
            BridgeCliMode::RegisterHost {
                extension_id: Some(PINNED_CHROMIUM_EXTENSION_ID.to_string())
            }
        );

        // Register host with extension id = syntax
        let reg_eq = vec![
            format!("--extension-id={PINNED_CHROMIUM_EXTENSION_ID}"),
            "--register-host".to_string(),
        ];
        assert_eq!(
            parse_cli_args(reg_eq).unwrap(),
            BridgeCliMode::RegisterHost {
                extension_id: Some(PINNED_CHROMIUM_EXTENSION_ID.to_string())
            }
        );

        // Register host with invalid extension id fails
        let reg_invalid = vec![
            "--register-host".to_string(),
            "--extension-id".to_string(),
            "bad_id".to_string(),
        ];
        assert!(parse_cli_args(reg_invalid).is_err());

        // Unregister host
        let unreg = vec!["--unregister-host".to_string()];
        assert_eq!(
            parse_cli_args(unreg).unwrap(),
            BridgeCliMode::UnregisterHost
        );

        // Ping
        let ping = vec!["--ping".to_string()];
        assert_eq!(parse_cli_args(ping).unwrap(), BridgeCliMode::Ping);
    }

    #[test]
    fn test_execute_ping_returns_pong() {
        let resp = execute_ping().expect("ping succeeds");
        match resp {
            Response::Pong {
                version,
                app_version,
                ..
            } => {
                assert_eq!(version, PROTOCOL_VERSION);
                assert_eq!(app_version, env!("CARGO_PKG_VERSION"));
            }
            _ => panic!("Expected Pong response"),
        }
    }

    fn make_test_temp_dir() -> PathBuf {
        let unique_name = format!(
            "test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        if let Ok(cargo_tmp) = std::env::var("CARGO_TARGET_TMPDIR") {
            let p = PathBuf::from(cargo_tmp).join(&unique_name);
            let _ = std::fs::create_dir_all(&p);
            return p;
        }
        let target_tmp = Path::new("target").join("test-tmp").join(&unique_name);
        let _ = std::fs::create_dir_all(&target_tmp);
        target_tmp
    }

    #[test]
    fn test_register_and_unregister_with_mock_registry() {
        let temp_dir = make_test_temp_dir();
        std::env::set_var("SAYL_LOCALAPPDATA", &temp_dir);

        let mut mock_reg = MockRegistry::default();
        let bridge_exe = temp_dir.join("sayl-bridge.exe");
        fs::write(&bridge_exe, b"dummy exe").expect("write dummy exe");

        let summary = register_host(&mut mock_reg, &bridge_exe, None).expect("registers host");
        assert_eq!(summary.status, "ok");
        assert_eq!(summary.registered_keys.len(), 6);

        // Verify all 6 keys were set in the mock registry
        assert!(mock_reg.key_exists(REG_CHROME));
        assert!(mock_reg.key_exists(REG_EDGE));
        assert!(mock_reg.key_exists(REG_BRAVE));
        assert!(mock_reg.key_exists(REG_OPERA));
        assert!(mock_reg.key_exists(REG_VIVALDI));
        assert!(mock_reg.key_exists(REG_FIREFOX));

        // Verify Chromium manifest was written and key points to it
        let chrom_path = chromium_manifest_path();
        assert!(chrom_path.exists());
        assert_eq!(
            mock_reg.keys.get(REG_CHROME).unwrap(),
            &chrom_path.to_string_lossy().to_string()
        );

        // Verify Firefox manifest was written and key points to it
        let ff_path = firefox_manifest_path();
        assert!(ff_path.exists());
        assert_eq!(
            mock_reg.keys.get(REG_FIREFOX).unwrap(),
            &ff_path.to_string_lossy().to_string()
        );

        // Now test unregister
        unregister_host(&mut mock_reg).expect("unregisters host");
        assert!(!mock_reg.key_exists(REG_CHROME));
        assert!(!mock_reg.key_exists(REG_EDGE));
        assert!(!mock_reg.key_exists(REG_BRAVE));
        assert!(!mock_reg.key_exists(REG_OPERA));
        assert!(!mock_reg.key_exists(REG_VIVALDI));
        assert!(!mock_reg.key_exists(REG_FIREFOX));
        assert!(!chrom_path.exists());
        assert!(!ff_path.exists());

        std::env::remove_var("SAYL_LOCALAPPDATA");
    }
}
