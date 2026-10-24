//! The stdio host loop: frames in, responses out.
//!
//! The loop holds no policy of its own beyond framing and dispatch, so it can be
//! driven by an in-memory transport in tests and by stdin/stdout in production.

use std::io::{Read, Write};

use crate::message::{Request, Response, PROTOCOL_VERSION};
use crate::protocol::{decode_frame, write_message, BridgeError, MAX_MESSAGE_BYTES};

/// Something that can turn a request into a response.
pub trait Dispatcher {
    /// Handles one request. Implementations must never panic on hostile input.
    fn handle(&mut self, req: Request) -> Response;
}

/// Answers `Ping` and refuses everything else; useful for transport tests.
pub struct EchoDispatcher;

impl Dispatcher for EchoDispatcher {
    fn handle(&mut self, req: Request) -> Response {
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
            _ => Response::error("not_implemented", "this dispatcher only answers Ping"),
        }
    }
}

/// The command line a browser passes when launching a native-messaging host.
///
/// Chrome appends the calling extension's origin as the first argument. It is
/// recorded for diagnostics only: it is **not** an authorisation token, because a
/// local process can pass the same argument.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandLine {
    /// The origin argument, when the browser supplied one.
    pub caller_origin: Option<String>,
    /// The parent window handle, when the browser supplied one.
    pub parent_window: Option<String>,
}

impl CommandLine {
    /// Parses the arguments after the executable name.
    pub fn from_args<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        Self {
            caller_origin: args.next(),
            parent_window: args.next(),
        }
    }
}

/// Serves frames from `reader` and writes responses to `writer`.
///
/// Returns the number of messages handled. Stops cleanly on EOF. A frame that
/// cannot be framed (oversized, truncated, zero-length) yields one error response
/// and ends the loop, because the stream is no longer aligned. A frame that frames
/// correctly but does not parse yields an error response and the loop continues.
pub fn serve<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    dispatcher: &mut dyn Dispatcher,
) -> Result<u64, BridgeError> {
    let mut handled = 0u64;
    loop {
        let payload = match decode_frame(&mut reader) {
            Ok(None) => return Ok(handled),
            Ok(Some(payload)) => payload,
            Err(err @ (BridgeError::FrameTooLarge { .. } | BridgeError::EmptyFrame)) => {
                let _ = write_message(&mut writer, &Response::error("bad_frame", err.to_string()));
                return Ok(handled);
            }
            Err(BridgeError::Io(msg)) => {
                let _ = write_message(&mut writer, &Response::error("io_error", msg.clone()));
                return Err(BridgeError::Io(msg));
            }
            Err(other) => return Err(other),
        };

        let response = match serde_json::from_slice::<Request>(&payload) {
            Ok(req) => dispatcher.handle(req),
            Err(err) => Response::error("malformed_json", err.to_string()),
        };

        write_message(&mut writer, &response)?;
        handled += 1;
    }
}

/// Largest frame the host will ever emit, for callers sizing their buffers.
pub fn max_outbound_frame() -> usize {
    MAX_MESSAGE_BYTES + 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Response;
    use std::io::Cursor;

    struct Recorder {
        seen: Vec<Request>,
        response: Option<Response>,
    }

    impl Dispatcher for Recorder {
        fn handle(&mut self, req: Request) -> Response {
            self.seen.push(req);
            self.response
                .clone()
                .unwrap_or_else(|| Response::error("empty", "no canned response"))
        }
    }

    fn frames(messages: &[&str]) -> Vec<u8> {
        let mut buf = Vec::new();
        for msg in messages {
            let body = msg.as_bytes();
            buf.extend_from_slice(&(body.len() as u32).to_le_bytes());
            buf.extend_from_slice(body);
        }
        buf
    }

    #[test]
    fn serves_until_eof() {
        let input = frames(&[
            r#"{"type":"Ping","version":1}"#,
            r#"{"type":"Ping","version":1}"#,
        ]);
        let mut out = Vec::new();
        let mut dispatcher = EchoDispatcher;
        let handled = serve(Cursor::new(input), &mut out, &mut dispatcher).unwrap();
        assert_eq!(handled, 2);

        // Two Pong frames are readable in order.
        let mut cursor = Cursor::new(out);
        for _ in 0..2 {
            let payload = decode_frame(&mut cursor).unwrap().unwrap();
            let resp: Response = serde_json::from_slice(&payload).unwrap();
            assert!(matches!(resp, Response::Pong { .. }));
        }
    }

    #[test]
    fn garbage_json_still_answers_and_keeps_going() {
        let input = frames(&[r#"not json at all"#, r#"{"type":"Ping","version":1}"#]);
        let mut out = Vec::new();
        let mut dispatcher = EchoDispatcher;
        let handled = serve(Cursor::new(input), &mut out, &mut dispatcher).unwrap();
        assert_eq!(handled, 2);

        let mut cursor = Cursor::new(out);
        let first: Response =
            serde_json::from_slice(&decode_frame(&mut cursor).unwrap().unwrap()).unwrap();
        let second: Response =
            serde_json::from_slice(&decode_frame(&mut cursor).unwrap().unwrap()).unwrap();
        assert!(matches!(first, Response::Error { .. }));
        assert!(matches!(second, Response::Pong { .. }));
    }

    #[test]
    fn unknown_request_type_is_an_error_response() {
        let input = frames(&[r#"{"type":"RunShell","cmd":"whoami"}"#]);
        let mut out = Vec::new();
        let mut dispatcher = EchoDispatcher;
        serve(Cursor::new(input), &mut out, &mut dispatcher).unwrap();
        let resp: Response =
            serde_json::from_slice(&decode_frame(Cursor::new(out)).unwrap().unwrap()).unwrap();
        assert!(matches!(resp, Response::Error { .. }));
    }

    #[test]
    fn oversized_frame_answers_then_stops() {
        let mut input = ((MAX_MESSAGE_BYTES + 1) as u32).to_le_bytes().to_vec();
        input.extend_from_slice(b"{}");
        input.extend_from_slice(&frames(&[r#"{"type":"Ping","version":1}"#]));
        let mut out = Vec::new();
        let mut dispatcher = EchoDispatcher;
        let handled = serve(Cursor::new(input), &mut out, &mut dispatcher).unwrap();
        assert_eq!(handled, 0);
        let resp: Response =
            serde_json::from_slice(&decode_frame(Cursor::new(out)).unwrap().unwrap()).unwrap();
        match resp {
            Response::Error { code, .. } => assert_eq!(code, "bad_frame"),
            other => panic!("expected bad_frame, got {other:?}"),
        }
    }

    #[test]
    fn command_line_records_the_origin_without_trusting_it() {
        let cmd = CommandLine::from_args(["chrome-extension://abc", "0"]);
        assert_eq!(cmd.caller_origin.as_deref(), Some("chrome-extension://abc"));
        assert_eq!(cmd.parent_window.as_deref(), Some("0"));
    }
}
