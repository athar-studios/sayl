//! Command-line argument parser definitions using Clap.

use std::path::PathBuf;
use clap::{Args, Parser, Subcommand};

use crate::messages::Language;

pub const EXIT_CODES_DOC: &str = "\
Exit codes:
  0  Success
  2  Usage error
  3  Task failed
  4  Cancelled
  5  Store or I/O error";

#[derive(Parser, Debug)]
#[command(
    name = "sayl",
    version = "0.1.0",
    about = "SAYL (سيل) - High-performance Windows-first download manager",
    after_help = EXIT_CODES_DOC
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Download destination directory [default: %USERPROFILE%\\Downloads\\SAYL]
    #[arg(global = true, long)]
    pub download_dir: Option<PathBuf>,

    /// Data and storage directory [default: <download-dir>\\.data]
    #[arg(global = true, long)]
    pub data_dir: Option<PathBuf>,

    /// Maximum concurrent connections per task (used for parallel ranged downloads)
    #[arg(global = true, long)]
    pub connections: Option<usize>,

    /// Maximum concurrent active download tasks
    #[arg(global = true, long)]
    pub concurrent: Option<usize>,

    /// Output results in JSON format
    #[arg(global = true, long)]
    pub json: bool,

    /// Suppress progress and informational output
    #[arg(global = true, short, long)]
    pub quiet: bool,

    /// Enable verbose diagnostic logging
    #[arg(global = true, short, long)]
    pub verbose: bool,

    /// Output language [default: ar]
    #[arg(global = true, long, value_enum, default_value = "ar")]
    pub lang: Language,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Add one or more URLs to the download queue
    Add(AddArgs),
    /// Start one or all queued download tasks
    Start(StartArgs),
    /// List all download tasks in tabular or JSON format
    List,
    /// Display detailed information about a specific task
    Info(IdArg),
    /// Pause an active or queued download task
    Pause(IdArg),
    /// Cancel an active or queued download task
    Cancel(IdArg),
    /// Remove a download task from storage
    Remove(RemoveArgs),
    /// Recover non-terminal tasks interrupted by crash or restart
    Recover,
    /// Stream live engine events as JSON until interrupted
    Events(EventsArgs),
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Target URL(s) to add to the download manager
    #[arg(required = true, num_args = 1..)]
    pub urls: Vec<String>,
}

#[derive(Args, Debug)]
pub struct StartArgs {
    /// Task ID or unique prefix to start
    #[arg(required_unless_present = "all", conflicts_with = "all")]
    pub id: Option<String>,

    /// Start every queued task
    #[arg(long)]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct IdArg {
    /// Full Task ID or unique prefix
    pub id: String,
}

#[derive(Args, Debug)]
pub struct RemoveArgs {
    /// Full Task ID or unique prefix
    pub id: String,

    /// Also delete the downloaded file and partial (.part) file from disk
    #[arg(long)]
    pub delete_file: bool,
}

#[derive(Args, Debug)]
pub struct EventsArgs {
    /// Automatically exit after N milliseconds of event silence
    #[arg(long)]
    pub idle_exit: Option<u64>,
}

/// Resolves the effective download directory.
pub fn resolve_download_dir(provided: Option<PathBuf>) -> PathBuf {
    if let Some(p) = provided {
        return p;
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        PathBuf::from(profile).join("Downloads").join("SAYL")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join("Downloads").join("SAYL")
    } else {
        PathBuf::from("downloads")
    }
}

/// Resolves the effective data directory.
pub fn resolve_data_dir(provided: Option<PathBuf>, download_dir: &std::path::Path) -> PathBuf {
    if let Some(p) = provided {
        return p;
    }
    download_dir.join(".data")
}
