//! SAYL desktop application executable discovery.

use std::path::{Path, PathBuf};

/// Resolves the absolute path to `sayl.exe` (or `sayl-desktop.exe` in dev environments).
///
/// Probes in order:
/// 1. Env `SAYL_APP_EXE`
/// 2. Registry `HKCU\Software\SAYL\InstallDir\sayl.exe`
/// 3. `%LOCALAPPDATA%\Programs\SAYL\sayl.exe`
/// 4. The bridge exe's own directory + `sayl.exe`
/// 5. That directory's parent + `sayl.exe`
/// 6. `..\..\sayl.exe` for a dev target dir (also checks `sayl-desktop.exe`)
pub fn find_app_exe() -> Option<PathBuf> {
    // 1. Env SAYL_APP_EXE (if explicitly set, honour it directly)
    if let Some(custom) = std::env::var_os("SAYL_APP_EXE") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Some(p);
        }
        let in_dir = p.join("sayl.exe");
        if in_dir.is_file() {
            return Some(in_dir);
        }
        // If explicitly set but neither is an existing file, return p so caller
        // attempts spawn and captures the exact OS error (e.g. Win32 file not found).
        return Some(p);
    }

    // 2. Registry HKCU\Software\SAYL\InstallDir\sayl.exe
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey(r"Software\SAYL") {
            if let Ok(install_dir) = key.get_value::<String, _>("InstallDir") {
                let p = PathBuf::from(&install_dir);
                if p.is_file() {
                    return Some(p);
                }
                let cand = p.join("sayl.exe");
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
        if let Ok(key) = hkcu.open_subkey(r"Software\SAYL\InstallDir") {
            if let Ok(path_str) = key.get_value::<String, _>("") {
                let p = PathBuf::from(&path_str);
                if p.is_file() {
                    return Some(p);
                }
                let cand = p.join("sayl.exe");
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
    }

    // 3. %LOCALAPPDATA%\Programs\SAYL\sayl.exe
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let cand = PathBuf::from(local_app_data)
            .join("Programs")
            .join("SAYL")
            .join("sayl.exe");
        if cand.is_file() {
            return Some(cand);
        }
    }

    // 4-6. Relative to bridge executable directory
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            // 4. The bridge exe's own directory + sayl.exe
            let cand4 = exe_dir.join("sayl.exe");
            if cand4.is_file() {
                return Some(cand4);
            }
            let cand4_desktop = exe_dir.join("sayl-desktop.exe");
            if cand4_desktop.is_file() {
                return Some(cand4_desktop);
            }

            // 5. That directory's parent + sayl.exe
            if let Some(parent) = exe_dir.parent() {
                let cand5 = parent.join("sayl.exe");
                if cand5.is_file() {
                    return Some(cand5);
                }
                let cand5_desktop = parent.join("sayl-desktop.exe");
                if cand5_desktop.is_file() {
                    return Some(cand5_desktop);
                }
            }

            // 6. ..\..\sayl.exe for a dev target dir
            let cand6 = exe_dir.join(r"..\..\sayl.exe");
            if cand6.is_file() {
                return Some(cand6);
            }
            let cand6_desktop = exe_dir.join(r"..\..\sayl-desktop.exe");
            if cand6_desktop.is_file() {
                return Some(cand6_desktop);
            }

            // Cargo target directory check: target/debug/sayl.exe
            let cand_dev = exe_dir.join(r"..\sayl.exe");
            if cand_dev.is_file() {
                return Some(cand_dev);
            }
        }
    }

    None
}

/// Helper for tests or custom resolution with explicit base path.
pub fn find_app_exe_with_base(base_dir: &Path) -> Option<PathBuf> {
    let cand = base_dir.join("sayl.exe");
    if cand.is_file() {
        return Some(cand);
    }
    let cand_desktop = base_dir.join("sayl-desktop.exe");
    if cand_desktop.is_file() {
        return Some(cand_desktop);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_app_exe_with_env_override() {
        let temp = tempfile::tempdir().unwrap();
        let dummy_exe = temp.path().join("sayl.exe");
        std::fs::write(&dummy_exe, b"dummy exe").unwrap();

        std::env::set_var("SAYL_APP_EXE", &dummy_exe);
        let found = find_app_exe();
        assert_eq!(found, Some(dummy_exe));
        std::env::remove_var("SAYL_APP_EXE");
    }
}
