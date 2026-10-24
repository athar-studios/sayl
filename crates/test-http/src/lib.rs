//! # sayl-test-http
//!
//! Deterministic local HTTP fixture server and failure mode simulator for the
//! SAYL download manager test harness.
//!
//! Provides simulation for all 27 failure modes described in MASTER-PLAN §8,
//! including partial content range validation, connection drops, corrupted headers,
//! rate limiting, and slow chunk streaming.
//!
//! ## Quick Example
//! ```no_run
//! use std::time::Duration;
//! use sayl_test_http::{HttpFixtureServer, Scenario, ExpectedRequest};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut server = HttpFixtureServer::with_scenario(Scenario::Ok206);
//!     server.set_chunk_size(1024);
//!     server.set_delay(Duration::from_millis(5));
//!
//!     let addr = server.start().await?;
//!     println!("Fixture server running at http://{}/", addr);
//!
//!     // Run client downloads ...
//!
//!     server.verify_requests(&[
//!         ExpectedRequest::get("/test.bin").with_range("bytes=0-499"),
//!     ]).await?;
//!
//!     server.shutdown();
//!     Ok(())
//! }
//! ```

pub mod body_gen;
pub mod scenarios;
pub mod server;

// Re-export primary API
pub use body_gen::{
    generate_bytes, generate_bytes_offset, verify_pattern, BodyGenerator, REFERENCE_SAMPLE_SIZE,
};
pub use scenarios::{Scenario, ScenarioConfig, ScenarioParseError};
pub use server::{
    ExpectedRequest, FixtureServerError, HttpFixtureServer, RecordedRequest, TryIntoScenario,
    VerificationError,
};

#[cfg(feature = "hyper")]
pub use hyper;
