//! The request and response vocabulary of the bridge, version 1.
//!
//! Requests are parsed into a closed enum, so an unknown `type` produces a typed
//! error response instead of being interpreted.

use serde::{Deserialize, Serialize};

/// The only protocol version this host implements.
pub const PROTOCOL_VERSION: u32 = 1;

/// A request from the browser extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    /// Handshake: the extension announces the protocol version it speaks.
    Ping {
        /// Version the extension expects.
        version: u32,
    },
    /// Ask the host to download a URL.
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
    fn unknown_type_is_a_parse_error() {
        let err = serde_json::from_str::<Request>(r#"{"type":"Execute","cmd":"rm -rf /"}"#)
            .unwrap_err();
        assert!(err.to_string().contains("unknown variant") || err.to_string().contains("Execute"));
    }

    #[test]
    fn error_response_carries_a_code() {
        let resp = Response::error("invalid_url", "scheme ftp is not allowed");
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "Error");
        assert_eq!(json["code"], "invalid_url");
    }
}
