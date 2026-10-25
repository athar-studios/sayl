//! sayl-bridge: Chromium and Firefox native messaging host binary for SAYL.

use std::io::{stdin, stdout};

use sayl_bridge::dispatch::EngineDispatcher;
use sayl_bridge::host::serve;
use sayl_bridge::install::{
    execute_ping, parse_cli_args, register_host, unregister_host, BridgeCliMode, WindowsRegistry,
};
use sayl_bridge::message::Response;
use sayl_bridge::protocol::write_message;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli_mode = match parse_cli_args(args) {
        Ok(mode) => mode,
        Err(err) => {
            eprintln!("Argument error: {err}");
            std::process::exit(1);
        }
    };

    match cli_mode {
        BridgeCliMode::RegisterHost { extension_id } => {
            let exe_path = match std::env::current_exe() {
                Ok(p) => p,
                Err(err) => {
                    eprintln!("Failed to resolve current exe path: {err}");
                    std::process::exit(1);
                }
            };
            let mut registry = WindowsRegistry;
            match register_host(&mut registry, &exe_path, extension_id.as_deref()) {
                Ok(summary) => {
                    let json = serde_json::to_string_pretty(&summary).unwrap_or_default();
                    println!("{json}");
                    std::process::exit(0);
                }
                Err(err) => {
                    eprintln!("Host registration failed: {err}");
                    std::process::exit(1);
                }
            }
        }
        BridgeCliMode::UnregisterHost => {
            let mut registry = WindowsRegistry;
            if let Err(err) = unregister_host(&mut registry) {
                eprintln!("Host unregistration failed: {err}");
                std::process::exit(1);
            }
            println!("{{\"status\":\"ok\",\"action\":\"unregistered\"}}");
            std::process::exit(0);
        }
        BridgeCliMode::Ping => match execute_ping() {
            Ok(pong) => {
                let json = serde_json::to_string(&pong).unwrap_or_default();
                println!("{json}");
                std::process::exit(0);
            }
            Err(err) => {
                eprintln!("Ping failed: {err}");
                std::process::exit(1);
            }
        },
        BridgeCliMode::Serve(_cmd_line) => {
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
    }
}
