use std::path::Path;
#[cfg(target_os = "windows")]
use std::process::Command;

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

/// Applies or removes the autostart registry entry via `reg.exe`.
pub fn set_autostart(enable: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if enable {
            let current_exe = std::env::current_exe()
                .map_err(|e| format!("Failed to get current executable path: {e}"))?;
            let cmd_val = build_autostart_command_value(&current_exe);

            let status = Command::new("reg.exe")
                .args([
                    "add",
                    REG_KEY,
                    "/v",
                    REG_VALUE_NAME,
                    "/t",
                    "REG_SZ",
                    "/d",
                    &cmd_val,
                    "/f",
                ])
                .status()
                .map_err(|e| format!("Failed to execute reg.exe: {e}"))?;

            if !status.success() {
                return Err(format!(
                    "reg.exe add failed with exit code: {:?}",
                    status.code()
                ));
            }
        } else {
            let status = Command::new("reg.exe")
                .args(["delete", REG_KEY, "/v", REG_VALUE_NAME, "/f"])
                .status()
                .map_err(|e| format!("Failed to execute reg.exe: {e}"))?;

            // If the key did not exist, reg.exe returns 1, but we treat that as successful removal
            if !status.success() && status.code() != Some(1) {
                return Err(format!(
                    "reg.exe delete failed with exit code: {:?}",
                    status.code()
                ));
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
