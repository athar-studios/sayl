//! Real engine dispatcher connecting native-messaging requests to `sayl_app::Engine`.

use std::path::PathBuf;

use crate::host::Dispatcher;
use crate::message::{Request, Response, PROTOCOL_VERSION};
use crate::validate::{validate_request_url, validate_suggested_filename};

/// Dispatches native-messaging requests to a real [`sayl_app::Engine`] instance.
pub struct EngineDispatcher {
    runtime: tokio::runtime::Runtime,
    engine: sayl_app::Engine,
    allow_private_hosts: bool,
}

impl EngineDispatcher {
    /// Creates a new dispatcher with default settings and specified directories.
    pub fn new(
        download_dir: impl Into<PathBuf>,
        store_dir: impl Into<PathBuf>,
    ) -> Result<Self, String> {
        Self::new_with_options(download_dir, store_dir, false)
    }

    /// Creates a new dispatcher with explicit directory paths and loopback policy.
    pub fn new_with_options(
        download_dir: impl Into<PathBuf>,
        store_dir: impl Into<PathBuf>,
        allow_private_hosts: bool,
    ) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("failed to build tokio runtime: {e}"))?;

        let dl_dir = download_dir.into();
        let st_dir = store_dir.into();
        let config = sayl_app::config::EngineConfig::default();
        let engine = sayl_app::Engine::new(&dl_dir, &st_dir, config).map_err(|e| {
            format!("failed to initialize engine at dl={dl_dir:?}, store={st_dir:?}: {e}")
        })?;

        Ok(Self {
            runtime,
            engine,
            allow_private_hosts,
        })
    }

    /// Creates a dispatcher wrapping an existing runtime and engine.
    pub fn with_engine(
        engine: sayl_app::Engine,
        runtime: tokio::runtime::Runtime,
        allow_private_hosts: bool,
    ) -> Self {
        Self {
            runtime,
            engine,
            allow_private_hosts,
        }
    }

    /// Constructs an [`EngineDispatcher`] from environment variables.
    ///
    /// Reads `SAYL_DOWNLOAD_DIR` and `SAYL_DATA_DIR`, falling back to `sayl-downloads`
    /// and `sayl-data` under the OS temporary directory if unset.
    /// Also reads `SAYL_ALLOW_PRIVATE_HOSTS` ("1" or "true") to permit LAN/loopback downloads.
    pub fn from_env() -> Result<Self, String> {
        let download_dir = std::env::var("SAYL_DOWNLOAD_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("sayl-downloads"));
        let data_dir = std::env::var("SAYL_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("sayl-data"));
        let allow_private_hosts = std::env::var("SAYL_ALLOW_PRIVATE_HOSTS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        Self::new_with_options(download_dir, data_dir, allow_private_hosts)
    }

    /// Reference to the underlying engine.
    pub fn engine(&self) -> &sayl_app::Engine {
        &self.engine
    }

    /// Reference to the Tokio runtime.
    pub fn runtime(&self) -> &tokio::runtime::Runtime {
        &self.runtime
    }

    fn handle_inner(&mut self, req: Request) -> Response {
        match req {
            Request::Ping { version } => {
                if version != PROTOCOL_VERSION {
                    return Response::error(
                        "unsupported_version",
                        format!("host speaks version {PROTOCOL_VERSION}"),
                    );
                }
                Response::Pong {
                    version: PROTOCOL_VERSION,
                    app_version: env!("CARGO_PKG_VERSION").to_string(),
                }
            }
            Request::Download {
                url,
                suggested_filename,
                ..
            } => {
                if let Err(err) = validate_request_url(&url, self.allow_private_hosts) {
                    return Response::error("invalid_url", err.to_string());
                }

                if let Some(ref name) = suggested_filename {
                    if let Err(err) = validate_suggested_filename(name) {
                        return Response::error("invalid_url", err.to_string());
                    }
                }

                match self.runtime.block_on(self.engine.add(&url)) {
                    Ok(id) => Response::Accepted {
                        task_id: id.to_string(),
                    },
                    Err(e) => Response::error("add_failed", e.to_string()),
                }
            }
            Request::Status { task_id } => {
                let parsed_id = match task_id.parse::<sayl_core::id::TaskId>() {
                    Ok(id) => id,
                    Err(_) => {
                        return Response::error(
                            "unknown_task",
                            format!("unparsable task id: {task_id}"),
                        )
                    }
                };

                match self.engine.get_task(parsed_id) {
                    Ok(Some(record)) => Response::Status {
                        state: record.state.to_string(),
                        bytes: record.written,
                    },
                    Ok(None) => {
                        Response::error("unknown_task", format!("task not found: {task_id}"))
                    }
                    Err(e) => Response::error("unknown_task", e.to_string()),
                }
            }
        }
    }
}

impl Dispatcher for EngineDispatcher {
    fn handle(&mut self, req: Request) -> Response {
        // Prevent any unexpected panic from escaping through the C-ABI or stdio loop
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.handle_inner(req))) {
            Ok(resp) => resp,
            Err(_) => Response::error(
                "internal_error",
                "dispatcher panicked while handling request",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sayl_test_http::{HttpFixtureServer, Scenario};

    fn make_temp_dir() -> PathBuf {
        if let Ok(cargo_tmp) = std::env::var("CARGO_TARGET_TMPDIR") {
            let p = PathBuf::from(cargo_tmp);
            let _ = std::fs::create_dir_all(&p);
            if let Ok(dir) = tempfile::tempdir_in(&p) {
                return dir.path().to_path_buf();
            }
        }
        let target_tmp = std::path::Path::new("target").join("test-tmp");
        let _ = std::fs::create_dir_all(&target_tmp);
        if let Ok(dir) = tempfile::tempdir_in(&target_tmp) {
            return dir.path().to_path_buf();
        }
        let unique = sayl_core::id::TaskId::new().to_string();
        let fallback = target_tmp.join(unique);
        let _ = std::fs::create_dir_all(&fallback);
        fallback
    }

    fn make_test_dispatcher(allow_private: bool) -> (EngineDispatcher, PathBuf, PathBuf) {
        let base = make_temp_dir();
        let dl = base.join("dl");
        let st = base.join("store");
        let dispatcher = EngineDispatcher::new_with_options(&dl, &st, allow_private)
            .expect("init test dispatcher");
        (dispatcher, dl, st)
    }

    fn cleanup_dirs(dl: PathBuf, st: PathBuf) {
        let _ = std::fs::remove_dir_all(&dl);
        let _ = std::fs::remove_dir_all(&st);
    }

    #[test]
    fn test_ping_pong_valid() {
        let (mut dispatcher, dl, st) = make_test_dispatcher(false);
        let resp = dispatcher.handle(Request::Ping { version: 1 });
        match resp {
            Response::Pong {
                version,
                app_version,
            } => {
                assert_eq!(version, 1);
                assert_eq!(app_version, env!("CARGO_PKG_VERSION"));
            }
            other => panic!("expected Pong, got {other:?}"),
        }
        cleanup_dirs(dl, st);
    }

    #[test]
    fn test_ping_unsupported_version() {
        let (mut dispatcher, dl, st) = make_test_dispatcher(false);
        let resp = dispatcher.handle(Request::Ping { version: 99 });
        match resp {
            Response::Error { code, .. } => assert_eq!(code, "unsupported_version"),
            other => panic!("expected Error unsupported_version, got {other:?}"),
        }
        cleanup_dirs(dl, st);
    }

    #[test]
    fn test_file_url_rejected_as_invalid_url() {
        let (mut dispatcher, dl, st) = make_test_dispatcher(true);
        let resp = dispatcher.handle(Request::Download {
            url: "file:///etc/passwd".to_string(),
            suggested_filename: None,
            referrer: None,
        });
        match resp {
            Response::Error { code, .. } => assert_eq!(code, "invalid_url"),
            other => panic!("expected Error invalid_url, got {other:?}"),
        }
        cleanup_dirs(dl, st);
    }

    #[test]
    fn test_unknown_task_id_returns_unknown_task() {
        let (mut dispatcher, dl, st) = make_test_dispatcher(false);

        // Case 1: unparsable ID
        let resp_unparsable = dispatcher.handle(Request::Status {
            task_id: "not-a-uuid".to_string(),
        });
        match resp_unparsable {
            Response::Error { code, .. } => assert_eq!(code, "unknown_task"),
            other => panic!("expected Error unknown_task, got {other:?}"),
        }

        // Case 2: valid UUID that does not exist in store
        let random_id = sayl_core::id::TaskId::new().to_string();
        let resp_nonexistent = dispatcher.handle(Request::Status { task_id: random_id });
        match resp_nonexistent {
            Response::Error { code, .. } => assert_eq!(code, "unknown_task"),
            other => panic!("expected Error unknown_task, got {other:?}"),
        }
        cleanup_dirs(dl, st);
    }

    #[test]
    fn test_http_download_accepted_and_status() {
        let (mut dispatcher, dl, st) = make_test_dispatcher(true);

        // Start a local HTTP fixture server (loopback only, does not touch external network)
        let (addr, _server_handle) = dispatcher.runtime().block_on(async {
            let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
            let addr = server.start().await.expect("start fixture server");
            (addr, server)
        });

        let test_url = format!("http://{addr}/test_archive.zip");
        let resp = dispatcher.handle(Request::Download {
            url: test_url,
            suggested_filename: Some("archive.zip".to_string()),
            referrer: None,
        });

        let task_id = match resp {
            Response::Accepted { task_id } => {
                assert!(!task_id.is_empty());
                task_id
            }
            other => panic!("expected Accepted, got {other:?}"),
        };

        // Check Status of the accepted task
        let status_resp = dispatcher.handle(Request::Status {
            task_id: task_id.clone(),
        });
        match status_resp {
            Response::Status { state, bytes } => {
                assert_eq!(state, "Queued");
                assert_eq!(bytes, 0);
            }
            other => panic!("expected Status Queued, got {other:?}"),
        }

        cleanup_dirs(dl, st);
    }

    #[test]
    fn test_probe_failure_on_unreachable_endpoint() {
        // Without an active server, the download task is accepted into the engine queue
        let (mut dispatcher, dl, st) = make_test_dispatcher(true);
        let resp = dispatcher.handle(Request::Download {
            url: "http://127.0.0.1:1/file.iso".to_string(),
            suggested_filename: None,
            referrer: None,
        });
        match resp {
            Response::Accepted { task_id } => {
                assert!(!task_id.is_empty());
            }
            Response::Error { code, .. } => assert_eq!(code, "add_failed"),
            other => panic!("expected Accepted or add_failed, got {other:?}"),
        }
        cleanup_dirs(dl, st);
    }

    #[test]
    fn test_from_env_constructs_dispatcher() {
        let base = make_temp_dir();
        let dl = base.join("env_dl");
        let st = base.join("env_st");
        std::env::set_var("SAYL_DOWNLOAD_DIR", &dl);
        std::env::set_var("SAYL_DATA_DIR", &st);
        let dispatcher = EngineDispatcher::from_env();
        assert!(
            dispatcher.is_ok(),
            "from_env should succeed with valid directories: {:?}",
            dispatcher.err()
        );
        cleanup_dirs(dl, st);
    }
}
