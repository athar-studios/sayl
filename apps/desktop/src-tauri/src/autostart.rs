use std::path::Path;

pub const REG_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
pub const REG_VALUE_NAME: &str = "SAYL";

/// Formats the registry command value: `"<absolute exe path>" --hidden`
pub fn build_autostart_command_value(exe_path: &Path) -> String {
    format!("\"{}\" --hidden", exe_path.display())
}

/// Detects whether `--hidden` is present in the command-line arguments.
pub fn is_hidden_arg<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|arg| arg.as_ref() == "--hidden")
}

/// Applies or removes the autostart registry entry via direct registry operations (`winreg`).
pub fn set_autostart(enable: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
            .map_err(|e| format!("Failed to open or create Run registry key: {e}"))?;

        if enable {
            let current_exe = std::env::current_exe()
                .map_err(|e| format!("Failed to get current executable path: {e}"))?;
            let cmd_val = build_autostart_command_value(&current_exe);
            key.set_value(REG_VALUE_NAME, &cmd_val)
                .map_err(|e| format!("Failed to set autostart registry value: {e}"))?;
        } else {
            match key.delete_value(REG_VALUE_NAME) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("Failed to delete autostart registry value: {e}")),
            }
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = enable;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_autostart_command_value() {
        let exe = PathBuf::from(r"C:\Program Files\SAYL\sayl.exe");
        let val = build_autostart_command_value(&exe);
        assert_eq!(val, r#""C:\Program Files\SAYL\sayl.exe" --hidden"#);

        let space_exe = PathBuf::from(r"C:\My Apps\Sub Dir\app.exe");
        let space_val = build_autostart_command_value(&space_exe);
        assert_eq!(space_val, r#""C:\My Apps\Sub Dir\app.exe" --hidden"#);
    }

    #[test]
    fn test_is_hidden_arg_detection() {
        assert!(is_hidden_arg(["sayl.exe", "--hidden"]));
        assert!(is_hidden_arg(["--hidden"]));
        assert!(is_hidden_arg(["sayl", "--other", "--hidden", "--more"]));

        assert!(!is_hidden_arg(["sayl.exe"]));
        assert!(!is_hidden_arg(["sayl.exe", "--headless"]));
        assert!(!is_hidden_arg(["sayl.exe", "hidden"]));
        let empty: [&str; 0] = [];
        assert!(!is_hidden_arg(empty));
    }
}
