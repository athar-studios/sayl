//! sayl-bridge: Chromium and Firefox native messaging host binary for SAYL.

use std::io::{stdin, stdout};

use sayl_bridge::dispatch::EngineDispatcher;
use sayl_bridge::host::serve;
use sayl_bridge::message::Response;
use sayl_bridge::protocol::write_message;

fn main() {
    let mut stdout = stdout().lock();
    let mut dispatcher = match EngineDispatcher::from_env() {
        Ok(d) => d,
        Err(err) => {
            let error_response = Response::error("init_failed", err);
            let _ = write_message(&mut stdout, &error_response);
            std::process::exit(1);
        }
    };

    let stdin = stdin().lock();
    if let Err(_err) = serve(stdin, &mut stdout, &mut dispatcher) {
        std::process::exit(1);
    }
}
