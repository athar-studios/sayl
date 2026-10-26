//! sayl-bridge: the browser native-messaging contract for SAYL.
//!
//! A browser extension asks the host to download a URL over stdio using the
//! Chrome/Firefox native-messaging framing (a little-endian `u32` length prefix
//! followed by that many bytes of UTF-8 JSON).
//!
//! Everything arriving on stdin is treated as untrusted input: frames are capped
//! at [`protocol::MAX_MESSAGE_BYTES`], JSON is parsed into a closed enum, URLs are
//! restricted to `http`/`https` with private networks blocked by default, and
//! suggested filenames are sanitised and confined to the download directory.

pub mod app_finder;
pub mod dispatch;
pub mod forward;
pub mod host;
pub mod install;
pub mod ipc;
pub mod log;
pub mod message;
pub mod protocol;
pub mod validate;

pub use app_finder::find_app_exe;
pub use dispatch::EngineDispatcher;
pub use forward::{build_app_launch_command, AppLaunchCommand, ForwardingDispatcher};
pub use host::{serve, CommandLine, Dispatcher, EchoDispatcher};
pub use ipc::{
    capture_ipc_path, capture_log_path, default_data_dir, read_ipc_info, CaptureIpcInfo,
};
pub use log::{append_log, sanitize_detail, sanitize_url};
pub use message::{BrowserCookie, Request, Response, PROTOCOL_VERSION};
pub use protocol::{decode_frame, encode_message, BridgeError, MAX_MESSAGE_BYTES};
pub use validate::{
    resolve_within, validate_request_url, validate_suggested_filename, MAX_URL_BYTES,
};
