//! Native-messaging framing: a 4-byte little-endian length prefix and a JSON body.
//!
//! The length is never trusted before it is checked against
//! [`MAX_MESSAGE_BYTES`], so a hostile peer cannot force a large allocation by
//! declaring a huge frame.

use std::io::{Read, Write};

/// Hard cap on one message, matching the platform convention of 1 MiB.
pub const MAX_MESSAGE_BYTES: usize = 1_048_576;

/// Everything that can go wrong while reading, validating or dispatching.
#[derive(Debug)]
pub enum BridgeError {
    /// The peer declared a frame larger than [`MAX_MESSAGE_BYTES`].
    FrameTooLarge { len: usize },
    /// The peer declared a zero-length frame.
    EmptyFrame,
    /// The JSON body could not be parsed into a known request.
    MalformedJson(String),
    /// The peer spoke a protocol version this host does not implement.
    UnsupportedVersion(u32),
    /// The URL was rejected by the URL policy.
    InvalidUrl(String),
    /// A suggested filename would escape the download directory.
    PathEscape,
    /// Reading from or writing to the transport failed.
    Io(String),
    /// The dispatcher refused or failed the request.
    Dispatcher(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FrameTooLarge { len } => {
                write!(
                    f,
                    "frame length {len} exceeds the {MAX_MESSAGE_BYTES} byte cap"
                )
            }
            Self::EmptyFrame => write!(f, "zero-length frame"),
            Self::MalformedJson(msg) => write!(f, "malformed message: {msg}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported protocol version {v}"),
            Self::InvalidUrl(msg) => write!(f, "invalid url: {msg}"),
            Self::PathEscape => write!(f, "path escapes the download directory"),
            Self::Io(msg) => write!(f, "io error: {msg}"),
            Self::Dispatcher(msg) => write!(f, "dispatcher error: {msg}"),
        }
    }
}

impl std::error::Error for BridgeError {}

impl From<std::io::Error> for BridgeError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

/// Serialises `msg` into a length-prefixed frame.
pub fn encode_message<T: serde::Serialize>(msg: &T) -> Result<Vec<u8>, BridgeError> {
    let body = serde_json::to_vec(msg).map_err(|e| BridgeError::MalformedJson(e.to_string()))?;
    if body.len() > MAX_MESSAGE_BYTES {
        return Err(BridgeError::FrameTooLarge { len: body.len() });
    }
    let mut out = Vec::with_capacity(body.len() + 4);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Writes `msg` as a frame and flushes.
pub fn write_message<W: Write, T: serde::Serialize>(
    writer: &mut W,
    msg: &T,
) -> Result<(), BridgeError> {
    let frame = encode_message(msg)?;
    writer.write_all(&frame)?;
    writer.flush()?;
    Ok(())
}

/// Reads one frame. `Ok(None)` means the peer closed the stream cleanly.
pub fn decode_frame(mut reader: impl Read) -> Result<Option<Vec<u8>>, BridgeError> {
    let mut len_buf = [0u8; 4];
    let mut read = 0usize;
    while read < 4 {
        match reader.read(&mut len_buf[read..]) {
            Ok(0) => {
                if read == 0 {
                    return Ok(None);
                }
                return Err(BridgeError::Io(format!(
                    "truncated length prefix: {read} of 4 bytes"
                )));
            }
            Ok(n) => read += n,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err.into()),
        }
    }

    let declared = u32::from_le_bytes(len_buf) as usize;
    if declared == 0 {
        return Err(BridgeError::EmptyFrame);
    }
    if declared > MAX_MESSAGE_BYTES {
        // Refuse before allocating anything based on the declared size.
        return Err(BridgeError::FrameTooLarge { len: declared });
    }

    let mut body = vec![0u8; declared];
    reader.read_exact(&mut body).map_err(|err| {
        BridgeError::Io(format!(
            "truncated frame body after the length prefix: {err}"
        ))
    })?;
    Ok(Some(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn round_trips_a_message() {
        let frame = encode_message(&serde_json::json!({"type":"Ping","version":1})).unwrap();
        let payload = decode_frame(Cursor::new(frame)).unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(value["type"], "Ping");
    }

    #[test]
    fn length_prefix_is_little_endian() {
        let frame = encode_message(&serde_json::json!({"a":1})).unwrap();
        let declared = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        assert_eq!(declared, frame.len() - 4);
    }

    #[test]
    fn clean_eof_returns_none() {
        assert!(decode_frame(Cursor::new(Vec::new())).unwrap().is_none());
    }

    #[test]
    fn zero_length_is_rejected() {
        let err = decode_frame(Cursor::new(vec![0, 0, 0, 0])).unwrap_err();
        assert!(matches!(err, BridgeError::EmptyFrame));
    }

    #[test]
    fn oversized_declaration_is_rejected_without_allocating() {
        let mut buf = ((MAX_MESSAGE_BYTES + 1) as u32).to_le_bytes().to_vec();
        buf.extend_from_slice(b"{}");
        let err = decode_frame(Cursor::new(buf)).unwrap_err();
        match err {
            BridgeError::FrameTooLarge { len } => assert_eq!(len, MAX_MESSAGE_BYTES + 1),
            other => panic!("expected FrameTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn short_body_is_an_io_error() {
        let mut buf = 16u32.to_le_bytes().to_vec();
        buf.extend_from_slice(b"{}");
        let err = decode_frame(Cursor::new(buf)).unwrap_err();
        assert!(matches!(err, BridgeError::Io(_)), "got {err:?}");
    }

    #[test]
    fn truncated_length_prefix_is_an_io_error() {
        let err = decode_frame(Cursor::new(vec![1, 0])).unwrap_err();
        assert!(matches!(err, BridgeError::Io(_)), "got {err:?}");
    }
}
