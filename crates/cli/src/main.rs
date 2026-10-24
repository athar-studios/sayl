//! SAYL CLI entry point binary.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let verbose = std::env::args().any(|a| a == "-v" || a == "--verbose");
    sayl_cli::init_tracing(verbose);
    sayl_cli::run(std::env::args_os()).await
}
