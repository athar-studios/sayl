//! CLI exit codes and conversions.

use std::process::ExitCode;

/// Standard exit codes for SAYL CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CliExit {
    /// Successful execution.
    Success = 0,
    /// Command-line usage or argument validation error.
    UsageError = 2,
    /// Download task failed.
    TaskFailed = 3,
    /// Download task was cancelled.
    Cancelled = 4,
    /// Store or file system I/O error.
    StoreIoError = 5,
}

impl CliExit {
    /// Returns the raw u8 numeric exit code.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns the signed integer exit code.
    pub fn as_i32(self) -> i32 {
        self as u8 as i32
    }
}

impl From<CliExit> for ExitCode {
    fn from(code: CliExit) -> Self {
        ExitCode::from(code.as_u8())
    }
}

impl From<sayl_core::error::CoreError> for CliExit {
    fn from(err: sayl_core::error::CoreError) -> Self {
        match err {
            sayl_core::error::CoreError::Validation { .. } => CliExit::UsageError,
            sayl_core::error::CoreError::Cancelled { .. } => CliExit::Cancelled,
            sayl_core::error::CoreError::Http { .. } => CliExit::TaskFailed,
            sayl_core::error::CoreError::Scheduler { .. } => CliExit::TaskFailed,
            sayl_core::error::CoreError::Io { .. } => CliExit::StoreIoError,
            sayl_core::error::CoreError::Storage { .. } => CliExit::StoreIoError,
            sayl_core::error::CoreError::Internal { .. } => CliExit::StoreIoError,
        }
    }
}
