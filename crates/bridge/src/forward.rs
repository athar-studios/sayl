//! Loopback forwarding dispatcher connecting native messaging requests to SAYL desktop app.

use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use serde_json::json;
use url::Url;

use crate::app_finder::find_app_exe;
use crate::host::Dispatcher;
use crate::ipc::{capture_ipc_path, read_ipc_info, CaptureIpcInfo};
use crate::log::{append_log, sanitize_url};
use crate::message::{Request, Response, PROTOCOL_VERSION};

const HEALTH_TIMEOUT: Duration = Duration::from_millis(700);
const APP_LAUNCH_POLL_TIMEOUT: Duration = Duration::from_secs(8);
const CAPTURE_TOTAL_TIMEOUT: Duration = Duration::from_secs(12);

#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(target_os = "windows")]
pub const DETACHED_PROCESS: u32 = 0x00000008;

/// A wrapper around [`Command`] tracking detached stdio and flags for testability.
#[derive(Debug)]
pub struct AppLaunchCommand {
    inner: Command,
    stdin_null: bool,
    stdout_null: bool,
    stderr_null: bool,
}

impl AppLaunchCommand {
    pub fn get_stdin(&self) -> Option<Stdio> {
        if self.stdin_null {
            Some(Stdio::null())
        } else {
            None
        }
    }

    pub fn get_stdout(&self) -> Option<Stdio> {
        if self.stdout_null {
            Some(Stdio::null())
        } else {
            None
        }
    }

    pub fn get_stderr(&self) -> Option<Stdio> {
        if self.stderr_null {
            Some(Stdio::null())
        } else {
            None
        }
    }

    pub fn spawn(&mut self) -> std::io::Result<std::process::Child> {
        self.inner.spawn()
    }

    pub fn into_inner(self) -> Command {
        self.inner
    }
}

impl Deref for AppLaunchCommand {
    type Target = Command;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for AppLaunchCommand {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl From<AppLaunchCommand> for Command {
    fn from(cmd: AppLaunchCommand) -> Self {
        cmd.inner
    }
}

/// Builds the app launch Command with detached stdio and flags so the child cannot touch the native messaging pipe.
pub fn build_app_launch_command(app_exe: &Path) -> AppLaunchCommand {
    let mut inner = Command::new(app_exe);
    inner
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(target_os = "windows")]
    {
        inner.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }

    AppLaunchCommand {
        inner,
        stdin_null: true,
        stdout_null: true,
        stderr_null: true,
    }
}

/// Dispatches native-messaging requests to the running SAYL desktop app via loopback HTTP IPC.
pub struct ForwardingDispatcher {
    runtime: tokio::runtime::Runtime,
    client: reqwest::Client,
    data_dir: Option<PathBuf>,
}

impl ForwardingDispatcher {
    /// Creates a new dispatcher using the default data directory.
    pub fn new() -> Self {
        Self::with_data_dir(None)
    }

    /// Creates a new dispatcher targeting a custom data directory (used in tests).
    pub fn with_data_dir(data_dir: Option<PathBuf>) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime for forwarding dispatcher");

        let client = reqwest::Client::builder()
            .build()
            .expect("failed to create reqwest client");

        Self {
            runtime,
            client,
            data_dir,
        }
    }

    fn log(&self, level: &str, event: &str, detail: serde_json::Value) {
        let _ = append_log(self.data_dir.as_deref(), level, event, detail);
    }

    fn check_health_sync(&self, port: u16) -> Result<(String, u32), String> {
        self.runtime.block_on(async {
            let url = format!("http://127.0.0.1:{port}/health");
            let resp = self
                .client
                .get(&url)
                .timeout(HEALTH_TIMEOUT)
                .send()
                .await
                .map_err(|e| format!("health check request failed: {e}"))?;

            if !resp.status().is_success() {
                return Err(format!("health check returned status {}", resp.status()));
            }

            let val: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| format!("failed to parse health response: {e}"))?;

            let version = val["app_version"].as_str().unwrap_or("0.1.0").to_string();
            let protocol = val["protocol"].as_u64().unwrap_or(1) as u32;
            Ok((version, protocol))
        })
    }

    fn read_ipc(&self) -> Result<CaptureIpcInfo, String> {
        let ipc_path = capture_ipc_path(self.data_dir.as_deref());
        read_ipc_info(&ipc_path)
    }

    fn ensure_app_reachable(&self) -> Result<CaptureIpcInfo, (String, String)> {
        // Step 1: Probe existing IPC file and health endpoint
        if let Ok(info) = self.read_ipc() {
            if self.check_health_sync(info.port).is_ok() {
                return Ok(info);
            }
        }

        // Step 2: Not reachable, locate app binary
        self.log(
            "info",
            "app_unreachable_launching",
            json!({"status": "attempting_spawn"}),
        );

        let app_exe = find_app_exe().ok_or_else(|| {
            let msg = "SAYL app executable could not be located".to_string();
            self.log("error", "app_locate_failed", json!({"error": &msg}));
            ("app_unreachable".to_string(), msg)
        })?;

        // Step 3: Spawn app with detached stdio so it doesn't inherit Chrome's native-messaging pipes
        let mut cmd = build_app_launch_command(&app_exe);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let msg = format!(
                    "failed to launch SAYL app ({}): {} (os error: {:?})",
                    app_exe.display(),
                    e,
                    e.raw_os_error()
                );
                self.log("error", "app_spawn_failed", json!({"error": &msg}));
                return Err(("app_unreachable".to_string(), msg));
            }
        };

        self.log(
            "info",
            "app_spawned",
            json!({"path": app_exe.to_string_lossy(), "pid": child.id()}),
        );

        // Step 4: Poll IPC file + /health for up to 8s
        let start = Instant::now();
        let mut last_err = "app did not start listening".to_string();

        while start.elapsed() < APP_LAUNCH_POLL_TIMEOUT {
            // Check if spawned process died prematurely
            if let Ok(Some(status)) = child.try_wait() {
                let msg = format!(
                    "app process ({}) terminated prematurely with status: {status}",
                    app_exe.display()
                );
                self.log("error", "app_terminated_early", json!({"error": &msg}));
                return Err(("app_unreachable".to_string(), msg));
            }

            std::thread::sleep(Duration::from_millis(150));
            if let Ok(info) = self.read_ipc() {
                match self.check_health_sync(info.port) {
                    Ok(_) => {
                        self.log(
                            "info",
                            "app_became_reachable",
                            json!({"elapsed_ms": start.elapsed().as_millis() as u64}),
                        );
                        return Ok(info);
                    }
                    Err(e) => {
                        last_err = e;
                    }
                }
            }
        }

        let msg = format!("app was launched but did not become reachable within 8s: {last_err}");
        self.log("error", "app_poll_timeout", json!({"error": &msg}));
        Err(("app_unreachable".to_string(), msg))
    }

    fn handle_capture(&mut self, req: Request) -> Response {
        let (version, url, final_url, filename, _referrer, cookies, user_agent, size, _mime) =
            match &req {
                Request::Capture {
                    version,
                    url,
                    final_url,
                    filename,
                    referrer,
                    cookies,
                    user_agent,
                    size,
                    mime,
                } => (
                    *version,
                    url.clone(),
                    final_url.clone(),
                    filename.clone(),
                    referrer.clone(),
                    cookies.clone(),
                    user_agent.clone(),
                    *size,
                    mime.clone(),
                ),
                _ => return Response::error("invalid_payload", "expected capture request"),
            };

        if version != PROTOCOL_VERSION {
            return Response::error("invalid_payload", format!("unsupported version {version}"));
        }

        // Validate URLs
        let parsed_url = match Url::parse(&url) {
            Ok(u) => u,
            Err(e) => return Response::error("invalid_url", format!("failed to parse url: {e}")),
        };
        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            return Response::error("invalid_url", "URL scheme must be http or https");
        }

        if let Some(ref fu) = final_url {
            let p_fu = match Url::parse(fu) {
                Ok(u) => u,
                Err(e) => {
                    return Response::error(
                        "invalid_url",
                        format!("failed to parse final_url: {e}"),
                    )
                }
            };
            if p_fu.scheme() != "http" && p_fu.scheme() != "https" {
                return Response::error("invalid_url", "final_url scheme must be http or https");
            }
        }

        self.log(
            "info",
            "capture_received",
            json!({
                "url_origin": sanitize_url(&url),
                "has_final_url": final_url.is_some(),
                "has_filename": filename.is_some(),
                "cookies_count": cookies.len(),
                "has_user_agent": user_agent.is_some(),
                "size": size,
            }),
        );

        let start_time = Instant::now();

        // Ensure desktop app is running and reachable
        let info = match self.ensure_app_reachable() {
            Ok(info) => info,
            Err((code, msg)) => return Response::error(code, msg),
        };

        // Remaining timeout for the POST /capture call
        let elapsed = start_time.elapsed();
        let remaining_timeout = CAPTURE_TOTAL_TIMEOUT.saturating_sub(elapsed);
        if remaining_timeout.is_zero() {
            let msg = "timeout reached before POST /capture could be dispatched".to_string();
            self.log("error", "capture_timeout", json!({"error": &msg}));
            return Response::error("timeout", msg);
        }

        let post_result: Result<String, (String, String)> = self.runtime.block_on(async {
            let capture_url = format!("http://127.0.0.1:{}/capture", info.port);
            let send_res = self
                .client
                .post(&capture_url)
                .header("Authorization", format!("Bearer {}", info.token))
                .header("Content-Type", "application/json")
                .timeout(remaining_timeout)
                .json(&req)
                .send()
                .await;

            let resp = match send_res {
                Ok(r) => r,
                Err(e) => {
                    if e.is_timeout() {
                        return Err(("timeout".to_string(), "POST /capture timed out".to_string()));
                    }
                    return Err((
                        "app_unreachable".to_string(),
                        format!("failed to connect to app capture endpoint: {e}"),
                    ));
                }
            };

            let status = resp.status();
            if !status.is_success() {
                let err_text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "app returned non-2xx error".to_string());

                let parsed_msg = serde_json::from_str::<serde_json::Value>(&err_text)
                    .ok()
                    .and_then(|v| v["message"].as_str().map(ToString::to_string))
                    .unwrap_or(err_text);

                return Err(("app_rejected".to_string(), parsed_msg));
            }

            let resp_json: serde_json::Value = resp.json().await.map_err(|e| {
                (
                    "app_rejected".to_string(),
                    format!("invalid response json: {e}"),
                )
            })?;

            let task_id = resp_json["task_id"]
                .as_str()
                .ok_or_else(|| {
                    (
                        "app_rejected".to_string(),
                        "missing task_id in app response".to_string(),
                    )
                })?
                .to_string();

            Ok(task_id)
        });

        match post_result {
            Ok(task_id) => {
                self.log(
                    "info",
                    "capture_accepted",
                    json!({
                        "task_id": &task_id,
                        "elapsed_ms": start_time.elapsed().as_millis() as u64
                    }),
                );
                Response::Accepted { task_id }
            }
            Err((code, msg)) => {
                self.log(
                    "error",
                    "capture_failed",
                    json!({"code": &code, "message": &msg}),
                );
                Response::error(code, msg)
            }
        }
    }
}

impl Default for ForwardingDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher for ForwardingDispatcher {
    fn handle(&mut self, req: Request) -> Response {
        match req {
            Request::Ping { version } => {
                if version != PROTOCOL_VERSION {
                    return Response::error(
                        "unsupported_version",
                        format!("host speaks version {PROTOCOL_VERSION}"),
                    );
                }

                // Check app connection without spawning
                let (connected, err_msg) = match self.read_ipc() {
                    Ok(info) => match self.check_health_sync(info.port) {
                        Ok(_) => (true, None),
                        Err(e) => (false, Some(e)),
                    },
                    Err(e) => (false, Some(e)),
                };

                Response::Pong {
                    version: PROTOCOL_VERSION,
                    app_version: env!("CARGO_PKG_VERSION").to_string(),
                    app_connected: Some(connected),
                    app_error: err_msg,
                }
            }
            Request::Health { version } => {
                if version != PROTOCOL_VERSION {
                    return Response::error(
                        "unsupported_version",
                        format!("host speaks version {PROTOCOL_VERSION}"),
                    );
                }

                // Health must NEVER start the app
                match self.read_ipc() {
                    Ok(info) => match self.check_health_sync(info.port) {
                        Ok((app_version, protocol)) => Response::Health {
                            version: PROTOCOL_VERSION,
                            app_connected: true,
                            app_version: Some(app_version),
                            app_protocol: Some(protocol),
                            app_error: None,
                        },
                        Err(e) => Response::Health {
                            version: PROTOCOL_VERSION,
                            app_connected: false,
                            app_version: None,
                            app_protocol: None,
                            app_error: Some(e),
                        },
                    },
                    Err(e) => Response::Health {
                        version: PROTOCOL_VERSION,
                        app_connected: false,
                        app_version: None,
                        app_protocol: None,
                        app_error: Some(e),
                    },
                }
            }
            Request::Log {
                level,
                message,
                detail,
            } => {
                self.log(&level, &message, detail.unwrap_or(serde_json::Value::Null));
                Response::Ok
            }
            Request::Download {
                url,
                suggested_filename,
                referrer,
            } => {
                // Map legacy Download request to Capture
                let cap = Request::Capture {
                    version: PROTOCOL_VERSION,
                    url,
                    final_url: None,
                    filename: suggested_filename,
                    referrer,
                    cookies: Vec::new(),
                    user_agent: None,
                    size: None,
                    mime: None,
                };
                self.handle_capture(cap)
            }
            Request::Capture { .. } => self.handle_capture(req),
            Request::Status { .. } => Response::error(
                "not_supported",
                "Status query is not supported; tasks are managed by the desktop app",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_app_launch_command_has_null_stdio() {
        let dummy = Path::new("sayl.exe");
        let cmd = build_app_launch_command(dummy);
        assert!(
            cmd.get_stdin().is_some(),
            "stdin must be redirected to null"
        );
        assert!(
            cmd.get_stdout().is_some(),
            "stdout must be redirected to null"
        );
        assert!(
            cmd.get_stderr().is_some(),
            "stderr must be redirected to null"
        );
    }
}
