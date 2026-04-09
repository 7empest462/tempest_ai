use thiserror::Error;
use miette::Diagnostic;

#[derive(Error, Diagnostic, Debug)]
pub enum FileError {
    #[error("File not found: {0}")]
    #[diagnostic(code(tempest::file::not_found), help("Check if the path exists and is correctly typed."))]
    NotFound(String),

    #[error("Permission denied: {0}")]
    #[diagnostic(code(tempest::file::permission_denied), help("Run with sudo or check file ownership."))]
    PermissionDenied(String),

    #[error("File too large: {path} ({size} bytes, max {max} bytes)")]
    #[diagnostic(code(tempest::file::too_large))]
    TooLarge { path: String, size: u64, max: u64 },

    #[error("IO error: {path} - {source}")]
    #[diagnostic(code(tempest::file::io))]
    Io { path: String, #[source] source: std::io::Error },
}

#[derive(Error, Diagnostic, Debug)]
pub enum ExecutionError {
    #[error("Command execution failed: {command} - {message}")]
    #[diagnostic(code(tempest::exec::command_failed))]
    CommandFailed { command: String, message: String },

    #[error("Timeout exceeded: {command}")]
    #[diagnostic(code(tempest::exec::timeout))]
    Timeout { command: String },
}

