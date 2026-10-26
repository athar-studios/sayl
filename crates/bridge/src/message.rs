//! The request and response vocabulary of the bridge, version 1.
//!
//! Requests are parsed into a closed enum, so an unknown `type` produces a typed
//! error response instead of being interpreted.

use serde::{Deserialize, Serialize};

/// The only protocol version this host implements.
pub const PROTOCOL_VERSION: u32 = 1;

/// A browser cookie associated with a download request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserCookie {
    pub name: String,
    pub value: String,
}

/// A request from the browser extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    /// Handshake: the extension announces the protocol version it speaks.
    Ping {
        /// Version the extension expects.
        version: u32,
    },
    /// Ask the host to download a URL (legacy format).
    Download {
        /// Absolute `http`/`https` URL.
        url: String,
        /// Optional name the extension would like the file saved under.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suggested_filename: Option<String>,
        /// Optional page the download was started from.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        referrer: Option<String>,
    },
    /// Rich capture request from browser extension.
    Capture {
        /// Protocol version.
        version: u32,
        /// Original download URL.
        url: String,
        /// Final resolved URL if redirected.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        final_url: Option<String>,
        /// Suggested filename.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        /// Referrer URL.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        referrer: Option<String>,
        /// Browser cookies.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        cookies: Vec<BrowserCookie>,
        /// User agent string.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        user_agent: Option<String>,
        /// Pre-determined file size if known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<u64>,
        /// MIME content type.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime: Option<String>,
    },
    /// Health check query from extension.
    Health {
        /// Protocol version.
        version: u32,
    },
    /// Log event from extension.
    Log {
        /// Severity level: "info", "warn", or "error".
        level: String,
        /// Message text.
        message: String,
        /// Detailed attributes.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<serde_json::Value>,
    },
    /// Ask about a task the host previously accepted.
    Status {
        /// Identifier returned by [`Response::Accepted`].
        task_id: String,
    },
}

/// The host's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    /// Handshake answer.
    Pong {
        /// Version the host speaks.
        version: u32,
        /// Host version string.
        app_version: String,
        /// Whether the SAYL desktop app is currently reachable.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app_connected: Option<bool>,
        /// Reason if app is not reachable.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app_error: Option<String>,
    },
    /// The download was accepted.
    Accepted {
        /// Identifier the extension can use with [`Request::Status`].
        task_id: String,
    },
    /// State of a previously accepted task.
    Status {
        /// Engine state name, for example `Downloading` or `Completed`.
        state: String,
        /// Bytes written so far.
        bytes: u64,
    },
    /// Health check response.
    Health {
        /// Protocol version.
        version: u32,
        /// Whether the desktop application is connected.
        app_connected: bool,
        /// Application version string (or null).
        app_version: Option<String>,
        /// Protocol version supported by app (or null).
        app_protocol: Option<u32>,
        /// Error description if app is not connected (or null).
        app_error: Option<String>,
    },
    /// Generic success response (for Log).
    Ok,
    /// Anything the host refused or failed.
    Error {
        /// Stable machine-readable code.
        code: String,
        /// Human-readable detail, never containing credentials.
        message: String,
    },
}

impl Response {
    /// Builds an error response.
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_ping() {
        let req: Request = serde_json::from_str(r#"{"type":"Ping","version":1}"#).unwrap();
        assert_eq!(req, Request::Ping { version: 1 });
    }

    #[test]
    fn parses_a_download_without_optional_fields() {
        let req: Request =
            serde_json::from_str(r#"{"type":"Download","url":"https://example.com/a.zip"}"#)
                .unwrap();
        match req {
            Request::Download {
                url,
                suggested_filename,
                referrer,
            } => {
                assert_eq!(url, "https://example.com/a.zip");
                assert!(suggested_filename.is_none());
                assert!(referrer.is_none());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_capture_request_full() {
        let json = r#"{
            "type": "Capture",
            "version": 1,
            "url": "https://example.com/file.zip",
            "final_url": "https://cdn.example.com/file.zip",
            "filename": "file.zip",
            "referrer": "https://page/",
            "cookies": [{"name":"session","value":"secret"}],
            "user_agent": "Mozilla/5.0",
            "size": 12345,
            "mime": "application/zip"
        }"#;
        let req: Request = serde_json::from_str(json).unwrap();
        match req {
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
            } => {
                assert_eq!(version, 1);
                assert_eq!(url, "https://example.com/file.zip");
                assert_eq!(
                    final_url.as_deref(),
                    Some("https://cdn.example.com/file.zip")
                );
                assert_eq!(filename.as_deref(), Some("file.zip"));
                assert_eq!(referrer.as_deref(), Some("https://page/"));
                assert_eq!(cookies.len(), 1);
                assert_eq!(cookies[0].name, "session");
                assert_eq!(cookies[0].value, "secret");
                assert_eq!(user_agent.as_deref(), Some("Mozilla/5.0"));
                assert_eq!(size, Some(12345));
                assert_eq!(mime.as_deref(), Some("application/zip"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_capture_request_minimal() {
        let json = r#"{"type":"Capture","version":1,"url":"https://example.com/f.zip"}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        match req {
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
            } => {
                assert_eq!(version, 1);
                assert_eq!(url, "https://example.com/f.zip");
                assert!(final_url.is_none());
                assert!(filename.is_none());
                assert!(referrer.is_none());
                assert!(cookies.is_empty());
                assert!(user_agent.is_none());
                assert!(size.is_none());
                assert!(mime.is_none());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_health_request() {
        let json = r#"{"type":"Health","version":1}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req, Request::Health { version: 1 });
    }

    #[test]
    fn parses_log_request() {
        let json = r#"{"type":"Log","level":"info","message":"download clicked","detail":{"source":"link"}}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        match req {
            Request::Log {
                level,
                message,
                detail,
            } => {
                assert_eq!(level, "info");
                assert_eq!(message, "download clicked");
                assert_eq!(detail.unwrap()["source"], "link");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn unknown_type_is_a_parse_error() {
        let err =
            serde_json::from_str::<Request>(r#"{"type":"Execute","cmd":"rm -rf /"}"#).unwrap_err();
        assert!(err.to_string().contains("unknown variant") || err.to_string().contains("Execute"));
    }

    #[test]
    fn error_response_carries_a_code() {
        let resp = Response::error("invalid_url", "scheme ftp is not allowed");
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "Error");
        assert_eq!(json["code"], "invalid_url");
    }

    #[test]
    fn health_response_serializes_null_for_none() {
        let resp = Response::Health {
            version: 1,
            app_connected: false,
            app_version: None,
            app_protocol: None,
            app_error: Some("connection refused".to_string()),
        };
        let json_str = serde_json::to_string(&resp).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(val["type"], "Health");
        assert_eq!(val["app_connected"], false);
        assert!(val["app_version"].is_null());
        assert!(val["app_protocol"].is_null());
        assert_eq!(val["app_error"], "connection refused");
    }

    #[test]
    fn ok_response_serializes_correctly() {
        let resp = Response::Ok;
        let json_str = serde_json::to_string(&resp).unwrap();
        assert_eq!(json_str, r#"{"type":"Ok"}"#);
    }
}
