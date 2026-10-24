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

pub mod dispatch;
pub mod host;
pub mod message;
pub mod protocol;
pub mod validate;

pub use dispatch::EngineDispatcher;
pub use host::{serve, CommandLine, Dispatcher, EchoDispatcher};
pub use message::{Request, Response, PROTOCOL_VERSION};
pub use protocol::{decode_frame, encode_message, BridgeError, MAX_MESSAGE_BYTES};
pub use validate::{
    resolve_within, validate_request_url, validate_suggested_filename, MAX_URL_BYTES,
};
