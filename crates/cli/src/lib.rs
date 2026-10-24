//! SAYL Command-Line Interface library core.
//!
//! Exposes the [`run`] entry point and module tree driving [`sayl_app::Engine`].

pub mod args;
pub mod commands;
pub mod exit;
pub mod messages;
pub mod output;
pub mod progress;

use std::ffi::OsString;
use std::process::ExitCode;

use clap::Parser;
use sayl_app::config::EngineConfig;
use sayl_app::Engine;

use crate::args::{resolve_data_dir, resolve_download_dir, Cli, Commands};
use crate::exit::CliExit;

/// Initializes tracing subscriber sending logs to stderr.
pub fn init_tracing(verbose: bool) {
    let default_filter = if verbose {
        "debug,sayl=trace"
    } else {
        "warn,sayl=info"
    };

    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
        )
        .with_writer(std::io::stderr)
        .try_init();
}

/// Executes the SAYL CLI with the given command-line arguments and returns an [`ExitCode`].
pub async fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let exit_code = run_inner(args).await;
    ExitCode::from(exit_code)
}

/// Inner execution function returning strongly-typed [`CliExit`].
pub async fn run_inner<I, T>(args: I) -> CliExit
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(err) => {
            let _ = err.print();
            return if err.use_stderr() {
                CliExit::UsageError
            } else {
                CliExit::Success
            };
        }
    };

    // Global flag validation
    if let Some(conn) = cli.connections {
        if conn == 0 {
            eprintln!("{}", messages::err_connections_zero(cli.lang));
            return CliExit::UsageError;
        }
    }

    if let Some(conc) = cli.concurrent {
        if conc == 0 {
            eprintln!("{}", messages::err_concurrent_zero(cli.lang));
            return CliExit::UsageError;
        }
    }

    // Engine configuration
    let mut config = EngineConfig::default();
    if let Some(conn) = cli.connections {
        config.max_connections = conn;
    }
    if let Some(conc) = cli.concurrent {
        config.max_concurrent_tasks = conc;
    }

    let download_dir = resolve_download_dir(cli.download_dir.clone());
    let data_dir = resolve_data_dir(cli.data_dir.clone(), &download_dir);

    let engine = match Engine::new(download_dir, data_dir, config) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("Failed to initialize download engine: {err}");
            return CliExit::from(err);
        }
    };

    let res = match &cli.command {
        Commands::Add(sub_args) => commands::handle_add(&engine, sub_args, &cli).await,
        Commands::Start(sub_args) => commands::handle_start(&engine, sub_args, &cli).await,
        Commands::List => commands::handle_list(&engine, &cli),
        Commands::Info(sub_args) => commands::handle_info(&engine, sub_args, &cli),
        Commands::Pause(sub_args) => commands::handle_pause(&engine, sub_args, &cli).await,
        Commands::Cancel(sub_args) => commands::handle_cancel(&engine, sub_args, &cli).await,
        Commands::Remove(sub_args) => commands::handle_remove(&engine, sub_args, &cli),
        Commands::Recover => commands::handle_recover(&engine, &cli).await,
        Commands::Events(sub_args) => commands::handle_events(&engine, sub_args, &cli).await,
    };

    match res {
        Ok(code) => code,
        Err(code) => code,
    }
}
