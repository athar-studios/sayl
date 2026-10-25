//! Browser detection and management for Chrome, Edge, Brave, Opera, Vivaldi, and Firefox.
//!
//! Provides pure, testable probe abstractions covering 32/64-bit and per-user
//! installations via registry App Paths/Uninstall keys and known executable paths.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Status report for an individual browser.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserStatus {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub host_registered: bool,
    pub connected: bool,
    pub extension_page_url: String,
    pub store_url: Option<String>,
    pub activate_kind: String,
}

/// Metadata and URL constants for supported browsers.
#[derive(Debug, Clone, Copy)]
pub struct BrowserMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub extension_page_url: &'static str,
    pub registry_key: &'static str,
}

pub const SUPPORTED_BROWSERS: &[BrowserMetadata] = &[
    BrowserMetadata {
        id: "chrome",
        name: "Google Chrome",
        extension_page_url: "chrome://extensions",
        registry_key: sayl_bridge::install::REG_CHROME,
    },
    BrowserMetadata {
        id: "edge",
        name: "Microsoft Edge",
        extension_page_url: "edge://extensions",
        registry_key: sayl_bridge::install::REG_EDGE,
    },
    BrowserMetadata {
        id: "brave",
        name: "Brave",
        extension_page_url: "brave://extensions",
        registry_key: sayl_bridge::install::REG_BRAVE,
    },
    BrowserMetadata {
        id: "opera",
        name: "Opera",
        extension_page_url: "opera://extensions",
        registry_key: sayl_bridge::install::REG_OPERA,
    },
    BrowserMetadata {
        id: "vivaldi",
        name: "Vivaldi",
        extension_page_url: "vivaldi://extensions",
        registry_key: sayl_bridge::install::REG_VIVALDI,
    },
    BrowserMetadata {
        id: "firefox",
        name: "Mozilla Firefox",
        extension_page_url: "about:addons",
        registry_key: sayl_bridge::install::REG_FIREFOX,
    },
];

/// Result of probing a browser's installation status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedBrowser {
    pub installed: bool,
    pub exe_path: Option<PathBuf>,
}

/// Abstraction for filesystem and registry queries to ensure deterministic testing.
pub trait BrowserProbeSource {
    fn file_exists(&self, path: &Path) -> bool;
    fn registry_key_exists(&self, key: &str) -> bool;
    fn registry_key_default_value(&self, key: &str) -> Option<String>;
    fn env_var(&self, name: &str) -> Option<String>;
}

/// Live production probe using the Windows filesystem and direct registry reads via `winreg`.
pub struct SystemBrowserProbe;

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

impl BrowserProbeSource for SystemBrowserProbe {
    fn file_exists(&self, path: &Path) -> bool {
        path.is_file() || path.exists()
    }

    fn registry_key_exists(&self, key: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            if let Some((root, subkey)) = parse_reg_key(key) {
                root.open_subkey(subkey).is_ok()
            } else {
                false
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = key;
            false
        }
    }

    fn registry_key_default_value(&self, key: &str) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            let (root, subkey) = parse_reg_key(key)?;
            let regkey = root.open_subkey(subkey).ok()?;
            let val: String = regkey.get_value("").ok()?;
            let clean = val.trim_matches('"').trim();
            if !clean.is_empty() {
                Some(clean.to_string())
            } else {
                None
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = key;
            None
        }
    }

    fn env_var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

/// Pure browser detection function checking known exe paths, App Paths, and Uninstall keys.
pub fn detect_browser_with_probe<P: BrowserProbeSource>(
    probe: &P,
    browser_id: &str,
) -> DetectedBrowser {
    let program_files = probe.env_var("ProgramFiles");
    let program_files_x86 = probe.env_var("ProgramFiles(x86)");
    let local_app_data = probe.env_var("LOCALAPPDATA");

    let mut candidate_paths: Vec<PathBuf> = Vec::new();
    let mut candidate_keys: Vec<&str> = Vec::new();
    let mut app_paths_keys: Vec<&str> = Vec::new();

    match browser_id {
        "chrome" => {
            if let Some(ref pf) = program_files {
                candidate_paths
                    .push(PathBuf::from(pf).join(r"Google\Chrome\Application\chrome.exe"));
            }
            if let Some(ref pfx86) = program_files_x86 {
                candidate_paths
                    .push(PathBuf::from(pfx86).join(r"Google\Chrome\Application\chrome.exe"));
            }
            if let Some(ref lad) = local_app_data {
                candidate_paths
                    .push(PathBuf::from(lad).join(r"Google\Chrome\Application\chrome.exe"));
            }
            app_paths_keys
                .push(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe");
            app_paths_keys
                .push(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe");
            app_paths_keys.push(
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe",
            );
            candidate_keys
                .push(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Google Chrome");
            candidate_keys
                .push(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Google Chrome");
        }
        "edge" => {
            if let Some(ref pfx86) = program_files_x86 {
                candidate_paths
                    .push(PathBuf::from(pfx86).join(r"Microsoft\Edge\Application\msedge.exe"));
            }
            if let Some(ref pf) = program_files {
                candidate_paths
                    .push(PathBuf::from(pf).join(r"Microsoft\Edge\Application\msedge.exe"));
            }
            if let Some(ref lad) = local_app_data {
                candidate_paths
                    .push(PathBuf::from(lad).join(r"Microsoft\Edge\Application\msedge.exe"));
            }
            app_paths_keys
                .push(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe");
            app_paths_keys
                .push(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe");
            app_paths_keys.push(
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe",
            );
            candidate_keys
                .push(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Microsoft Edge");
            candidate_keys
                .push(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Microsoft Edge");
        }
        "brave" => {
            if let Some(ref pf) = program_files {
                candidate_paths.push(
                    PathBuf::from(pf).join(r"BraveSoftware\Brave-Browser\Application\brave.exe"),
                );
            }
            if let Some(ref pfx86) = program_files_x86 {
                candidate_paths.push(
                    PathBuf::from(pfx86).join(r"BraveSoftware\Brave-Browser\Application\brave.exe"),
                );
            }
            if let Some(ref lad) = local_app_data {
                candidate_paths.push(
                    PathBuf::from(lad).join(r"BraveSoftware\Brave-Browser\Application\brave.exe"),
                );
            }
            app_paths_keys
                .push(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\brave.exe");
            app_paths_keys
                .push(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\brave.exe");
            app_paths_keys.push(
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths\brave.exe",
            );
            candidate_keys.push(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Brave");
            candidate_keys.push(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Brave");
            candidate_keys.push(r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\BraveSoftware Brave-Browser");
            candidate_keys.push(r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\BraveSoftware Brave-Browser");
        }
        "opera" => {
            if let Some(ref lad) = local_app_data {
                candidate_paths.push(PathBuf::from(lad).join(r"Programs\Opera\opera.exe"));
                candidate_paths.push(PathBuf::from(lad).join(r"Programs\Opera\launcher.exe"));
            }
            if let Some(ref pf) = program_files {
                candidate_paths.push(PathBuf::from(pf).join(r"Opera\opera.exe"));
                candidate_paths.push(PathBuf::from(pf).join(r"Opera\launcher.exe"));
            }
            if let Some(ref pfx86) = program_files_x86 {
                candidate_paths.push(PathBuf::from(pfx86).join(r"Opera\opera.exe"));
                candidate_paths.push(PathBuf::from(pfx86).join(r"Opera\launcher.exe"));
            }
            app_paths_keys
                .push(r"HKCU\Software\Microsoft\Windows\CurrentVersion\App Paths\opera.exe");
            app_paths_keys
                .push(r"HKLM\Software\Microsoft\Windows\CurrentVersion\App Paths\opera.exe");
            candidate_keys
                .push(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Opera Stable");
            candidate_keys
                .push(r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\Opera Stable");
        }
        "vivaldi" => {
            if let Some(ref lad) = local_app_data {
                candidate_paths.push(PathBuf::from(lad).join(r"Vivaldi\Application\vivaldi.exe"));
            }
            if let Some(ref pf) = program_files {
                candidate_paths.push(PathBuf::from(pf).join(r"Vivaldi\Application\vivaldi.exe"));
            }
            if let Some(ref pfx86) = program_files_x86 {
                candidate_paths.push(PathBuf::from(pfx86).join(r"Vivaldi\Application\vivaldi.exe"));
            }
            app_paths_keys
                .push(r"HKCU\Software\Microsoft\Windows\CurrentVersion\App Paths\vivaldi.exe");
            app_paths_keys
                .push(r"HKLM\Software\Microsoft\Windows\CurrentVersion\App Paths\vivaldi.exe");
            candidate_keys
                .push(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Vivaldi");
            candidate_keys
                .push(r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\Vivaldi");
        }
        "firefox" => {
            if let Some(ref pf) = program_files {
                candidate_paths.push(PathBuf::from(pf).join(r"Mozilla Firefox\firefox.exe"));
            }
            if let Some(ref pfx86) = program_files_x86 {
                candidate_paths.push(PathBuf::from(pfx86).join(r"Mozilla Firefox\firefox.exe"));
            }
            if let Some(ref lad) = local_app_data {
                candidate_paths.push(PathBuf::from(lad).join(r"Mozilla Firefox\firefox.exe"));
            }
            app_paths_keys
                .push(r"HKLM\Software\Microsoft\Windows\CurrentVersion\App Paths\firefox.exe");
            app_paths_keys
                .push(r"HKCU\Software\Microsoft\Windows\CurrentVersion\App Paths\firefox.exe");
            app_paths_keys.push(
                r"HKLM\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths\firefox.exe",
            );
            candidate_keys.push(r"HKLM\Software\Mozilla\Mozilla Firefox");
            candidate_keys
                .push(r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\Mozilla Firefox");
            candidate_keys
                .push(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Mozilla Firefox");
        }
        _ => {
            return DetectedBrowser {
                installed: false,
                exe_path: None,
            }
        }
    }

    // 1. Check known candidate exe paths on disk
    for path in candidate_paths {
        if probe.file_exists(&path) {
            return DetectedBrowser {
                installed: true,
                exe_path: Some(path),
            };
        }
    }

    // 2. Check App Paths in registry and extract default value if found
    for key in app_paths_keys {
        if probe.registry_key_exists(key) {
            if let Some(val) = probe.registry_key_default_value(key) {
                let p = PathBuf::from(&val);
                return DetectedBrowser {
                    installed: true,
                    exe_path: Some(p),
                };
            }
            return DetectedBrowser {
                installed: true,
                exe_path: None,
            };
        }
    }

    // 3. Check Uninstall / Application registry keys
    for key in candidate_keys {
        if probe.registry_key_exists(key) {
            return DetectedBrowser {
                installed: true,
                exe_path: None,
            };
        }
    }

    DetectedBrowser {
        installed: false,
        exe_path: None,
    }
}

/// Collects status information for all 6 supported browsers.
pub fn collect_browser_statuses<
    P: BrowserProbeSource,
    R: sayl_bridge::install::RegistryOperations,
>(
    probe: &P,
    registry: &R,
    ping_ok: bool,
) -> Vec<BrowserStatus> {
    SUPPORTED_BROWSERS
        .iter()
        .map(|meta| {
            let detected = detect_browser_with_probe(probe, meta.id);
            let manifest_path = if meta.id == "firefox" {
                sayl_bridge::install::firefox_manifest_path()
            } else {
                sayl_bridge::install::chromium_manifest_path()
            };
            let manifest_str = manifest_path.to_string_lossy().to_string();
            let host_registered = registry.key_has_default(meta.registry_key, Some(&manifest_str));
            let connected = ping_ok && host_registered;

            BrowserStatus {
                id: meta.id.to_string(),
                name: meta.name.to_string(),
                installed: detected.installed,
                host_registered,
                connected,
                extension_page_url: meta.extension_page_url.to_string(),
                store_url: None,
                activate_kind: "guided".to_string(),
            }
        })
        .collect()
}

/// Opens the extensions page in the given browser.
pub fn launch_browser_extensions_page<P: BrowserProbeSource>(
    probe: &P,
    browser_id: &str,
) -> Result<(), String> {
    let meta = SUPPORTED_BROWSERS
        .iter()
        .find(|b| b.id == browser_id)
        .ok_or_else(|| format!("Unknown browser id '{browser_id}'"))?;

    let detected = detect_browser_with_probe(probe, browser_id);
    if !detected.installed {
        return Err(format!("Browser '{}' is not installed", meta.name));
    }

    let exe = if let Some(path) = detected.exe_path {
        path
    } else {
        match browser_id {
            "chrome" => PathBuf::from("chrome.exe"),
            "edge" => PathBuf::from("msedge.exe"),
            "brave" => PathBuf::from("brave.exe"),
            "opera" => PathBuf::from("opera.exe"),
            "vivaldi" => PathBuf::from("vivaldi.exe"),
            "firefox" => PathBuf::from("firefox.exe"),
            _ => return Err(format!("Unknown browser executable for '{browser_id}'")),
        }
    };

    let mut cmd = std::process::Command::new(&exe);
    cmd.arg(meta.extension_page_url);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.spawn()
        .map_err(|e| format!("Failed to launch {} extensions page: {e}", meta.name))?;

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    /// In-memory mock browser probe environment.
    #[derive(Debug, Default, Clone)]
    pub struct MockBrowserProbe {
        pub files: HashSet<PathBuf>,
        pub registry_keys: HashSet<String>,
        pub registry_values: HashMap<String, String>,
        pub env: HashMap<String, String>,
    }

    impl MockBrowserProbe {
        pub fn new() -> Self {
            let mut env = HashMap::new();
            env.insert("ProgramFiles".to_string(), r"C:\Program Files".to_string());
            env.insert(
                "ProgramFiles(x86)".to_string(),
                r"C:\Program Files (x86)".to_string(),
            );
            env.insert(
                "LOCALAPPDATA".to_string(),
                r"C:\Users\User\AppData\Local".to_string(),
            );
            Self {
                files: HashSet::new(),
                registry_keys: HashSet::new(),
                registry_values: HashMap::new(),
                env,
            }
        }
    }

    impl BrowserProbeSource for MockBrowserProbe {
        fn file_exists(&self, path: &Path) -> bool {
            self.files.contains(path)
        }

        fn registry_key_exists(&self, key: &str) -> bool {
            let lower = key.to_lowercase();
            self.registry_keys.iter().any(|k| k.to_lowercase() == lower)
        }

        fn registry_key_default_value(&self, key: &str) -> Option<String> {
            let lower = key.to_lowercase();
            for (k, v) in &self.registry_values {
                if k.to_lowercase() == lower {
                    return Some(v.clone());
                }
            }
            None
        }

        fn env_var(&self, name: &str) -> Option<String> {
            self.env.get(name).cloned()
        }
    }

    #[test]
    fn test_browser_not_installed() {
        let probe = MockBrowserProbe::new();
        for meta in SUPPORTED_BROWSERS {
            let detected = detect_browser_with_probe(&probe, meta.id);
            assert!(
                !detected.installed,
                "Browser {} should not be installed",
                meta.id
            );
            assert!(detected.exe_path.is_none());
        }
    }

    #[test]
    fn test_chrome_detection_machine_and_per_user() {
        // 1. 64-bit Program Files machine install
        let mut probe64 = MockBrowserProbe::new();
        let chrome64 = PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe");
        probe64.files.insert(chrome64.clone());
        let res64 = detect_browser_with_probe(&probe64, "chrome");
        assert!(res64.installed);
        assert_eq!(res64.exe_path, Some(chrome64));

        // 2. 32-bit Program Files (x86) install
        let mut probe32 = MockBrowserProbe::new();
        let chrome32 =
            PathBuf::from(r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe");
        probe32.files.insert(chrome32.clone());
        let res32 = detect_browser_with_probe(&probe32, "chrome");
        assert!(res32.installed);
        assert_eq!(res32.exe_path, Some(chrome32));

        // 3. Per-user LocalAppData install
        let mut probe_user = MockBrowserProbe::new();
        let chrome_user =
            PathBuf::from(r"C:\Users\User\AppData\Local\Google\Chrome\Application\chrome.exe");
        probe_user.files.insert(chrome_user.clone());
        let res_user = detect_browser_with_probe(&probe_user, "chrome");
        assert!(res_user.installed);
        assert_eq!(res_user.exe_path, Some(chrome_user));

        // 4. Registry App Paths fallback
        let mut probe_reg = MockBrowserProbe::new();
        let reg_key = r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe";
        probe_reg.registry_keys.insert(reg_key.to_string());
        probe_reg.registry_values.insert(
            reg_key.to_string(),
            r"D:\Custom\Chrome\chrome.exe".to_string(),
        );
        let res_reg = detect_browser_with_probe(&probe_reg, "chrome");
        assert!(res_reg.installed);
        assert_eq!(
            res_reg.exe_path,
            Some(PathBuf::from(r"D:\Custom\Chrome\chrome.exe"))
        );
    }

    #[test]
    fn test_edge_detection() {
        let mut probe = MockBrowserProbe::new();
        let edge_path =
            PathBuf::from(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe");
        probe.files.insert(edge_path.clone());
        let res = detect_browser_with_probe(&probe, "edge");
        assert!(res.installed);
        assert_eq!(res.exe_path, Some(edge_path));
    }

    #[test]
    fn test_brave_detection() {
        let mut probe = MockBrowserProbe::new();
        let brave_path =
            PathBuf::from(r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe");
        probe.files.insert(brave_path.clone());
        let res = detect_browser_with_probe(&probe, "brave");
        assert!(res.installed);
        assert_eq!(res.exe_path, Some(brave_path));
    }

    #[test]
    fn test_opera_detection_per_user_programs() {
        let mut probe = MockBrowserProbe::new();
        let opera_path = PathBuf::from(r"C:\Users\User\AppData\Local\Programs\Opera\opera.exe");
        probe.files.insert(opera_path.clone());
        let res = detect_browser_with_probe(&probe, "opera");
        assert!(res.installed);
        assert_eq!(res.exe_path, Some(opera_path));
    }

    #[test]
    fn test_vivaldi_detection_per_user() {
        let mut probe = MockBrowserProbe::new();
        let vivaldi_path =
            PathBuf::from(r"C:\Users\User\AppData\Local\Vivaldi\Application\vivaldi.exe");
        probe.files.insert(vivaldi_path.clone());
        let res = detect_browser_with_probe(&probe, "vivaldi");
        assert!(res.installed);
        assert_eq!(res.exe_path, Some(vivaldi_path));
    }

    #[test]
    fn test_firefox_detection_and_uninstall_key() {
        let mut probe = MockBrowserProbe::new();
        let ff_key = r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\Mozilla Firefox";
        probe.registry_keys.insert(ff_key.to_string());
        let res = detect_browser_with_probe(&probe, "firefox");
        assert!(res.installed);
    }

    #[test]
    fn test_collect_browser_statuses_shape_and_connected_logic() {
        let mut probe = MockBrowserProbe::new();
        let chrome_path = PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe");
        probe.files.insert(chrome_path);

        let mut mock_reg = sayl_bridge::install::MockRegistry::default();
        let chrom_manifest_str = sayl_bridge::install::chromium_manifest_path()
            .to_string_lossy()
            .to_string();
        // Register chrome key
        mock_reg.keys.insert(
            sayl_bridge::install::REG_CHROME.to_string(),
            chrom_manifest_str,
        );

        // Ping succeeded = true
        let statuses = collect_browser_statuses(&probe, &mock_reg, true);
        assert_eq!(statuses.len(), 6);

        let chrome_st = statuses
            .iter()
            .find(|s| s.id == "chrome")
            .expect("chrome status");
        assert!(chrome_st.installed);
        assert!(chrome_st.host_registered);
        assert!(chrome_st.connected); // ping succeeds AND host_registered
        assert_eq!(chrome_st.extension_page_url, "chrome://extensions");
        assert_eq!(chrome_st.activate_kind, "guided");
        assert!(chrome_st.store_url.is_none());

        let edge_st = statuses
            .iter()
            .find(|s| s.id == "edge")
            .expect("edge status");
        assert!(!edge_st.installed);
        assert!(!edge_st.host_registered);
        assert!(!edge_st.connected);

        // If ping failed, connected must be false even if registered
        let statuses_no_ping = collect_browser_statuses(&probe, &mock_reg, false);
        let chrome_no_ping = statuses_no_ping.iter().find(|s| s.id == "chrome").unwrap();
        assert!(chrome_no_ping.host_registered);
        assert!(!chrome_no_ping.connected);
    }

    #[test]
    fn test_launch_uninstalled_browser_returns_error() {
        let probe = MockBrowserProbe::new();
        let err = launch_browser_extensions_page(&probe, "chrome").unwrap_err();
        assert!(err.contains("is not installed"));
    }
}
