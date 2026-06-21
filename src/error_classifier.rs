use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorClass {
    /// Transient failure, worth retrying automatically (e.g. network timeout).
    Retryable,
    /// System-level barrier that might be overcome with a different strategy (e.g. sudo).
    Recoverable,
    /// Logical failure or missing resource, model must handle (e.g. file not found).
    Fatal,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CategorizedError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Address/Port already in use: {0}")]
    AddressInUse(String),

    #[error("Network connection timeout: {0}")]
    NetworkTimeout(String),

    #[error("Remote server temporary error: {0}")]
    NetworkService(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Disk full or write failure: {0}")]
    DiskSpace(String),

    #[error("Command not found or bad syntax: {0}")]
    CommandNotFound(String),

    #[error("Compiler or linter error: {0}")]
    CompileError(String),

    #[error("Generic tool error: {0}")]
    Generic(String),
}

impl CategorizedError {
    pub fn class(&self) -> ErrorClass {
        match self {
            Self::PermissionDenied(_) | Self::AddressInUse(_) => ErrorClass::Recoverable,
            Self::NetworkTimeout(_) | Self::NetworkService(_) => ErrorClass::Retryable,
            _ => ErrorClass::Fatal,
        }
    }

    pub fn tip(&self) -> Option<String> {
        match self {
            Self::PermissionDenied(_) => Some(
                "This looks like a permission issue. You may need to ask the user for elevated privileges (root/sudo) or use a different path.".to_string()
            ),
            Self::AddressInUse(_) => Some(
                "The target port/address is already in use. You might want to locate the process holding the port using a utility like lsof/netstat, kill it, or choose a different port.".to_string()
            ),
            Self::NetworkTimeout(_) => Some(
                "Network connection timed out. The operation will be automatically retried with exponential backoff.".to_string()
            ),
            Self::NetworkService(msg) => Some(format!(
                "The remote server returned a temporary error ({}). Retrying might resolve this.",
                msg
            )),
            Self::NotFound(_) => Some(
                "The requested file, directory, or resource does not exist. Check that the path is correct or create the file before reading it.".to_string()
            ),
            Self::DiskSpace(_) => Some(
                "No space left on device or the filesystem is write-protected. Check disk space and write permissions.".to_string()
            ),
            Self::CommandNotFound(_) => Some(
                "The command or executable was not found on the system. Ensure the command-line utility is installed and available in the system PATH.".to_string()
            ),
            Self::CompileError(_) => Some(
                "A compiler, linter, or syntax check failed. Analyze the compiler output and modify the source code to resolve syntax/type issues.".to_string()
            ),
            Self::Generic(_) => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ErrorClassification {
    pub class: ErrorClass,
    pub category: String,
    pub tip: Option<String>,
}

/// Classifies a tool error based on the tool name and the error message.
pub fn classify_error(_tool_name: &str, error_msg: &str) -> ErrorClassification {
    let msg = error_msg.to_lowercase();

    let categorized = if msg.contains("timed out")
        || msg.contains("timeout")
        || msg.contains("temporary failure in name resolution")
        || msg.contains("network is unreachable")
        || msg.contains("broken pipe")
        || msg.contains("connection reset")
    {
        CategorizedError::NetworkTimeout(error_msg.to_string())
    } else if msg.contains("connection refused")
        || msg.contains("reset by peer")
        || msg.contains("error 400")
        || msg.contains("error 429")
        || msg.contains("error 500")
        || msg.contains("error 503")
        || msg.contains("error 502")
        || msg.contains("error 504")
    {
        CategorizedError::NetworkService(error_msg.to_string())
    } else if msg.contains("permission denied")
        || msg.contains("eacces")
        || msg.contains("operation not permitted")
        || msg.contains("sudo: a password is required")
    {
        CategorizedError::PermissionDenied(error_msg.to_string())
    } else if msg.contains("address already in use") || msg.contains("eaddrinuse") {
        CategorizedError::AddressInUse(error_msg.to_string())
    } else if msg.contains("no such file") || msg.contains("not found") || msg.contains("enoent") {
        CategorizedError::NotFound(error_msg.to_string())
    } else if msg.contains("no space left on device")
        || msg.contains("enospc")
        || msg.contains("read-only file system")
    {
        CategorizedError::DiskSpace(error_msg.to_string())
    } else if msg.contains("command not found") || msg.contains("sh: ") || msg.contains("bash: ") {
        CategorizedError::CommandNotFound(error_msg.to_string())
    } else if msg.contains("error[e")
        || msg.contains("failed to compile")
        || msg.contains("borrow check")
    {
        CategorizedError::CompileError(error_msg.to_string())
    } else {
        CategorizedError::Generic(error_msg.to_string())
    };

    ErrorClassification {
        class: categorized.class(),
        category: format!("{:?}", categorized)
            .split('(')
            .next()
            .unwrap_or("Generic")
            .to_string(),
        tip: categorized.tip(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_classification() {
        let err1 = classify_error("run_command", "connection refused");
        assert_eq!(err1.class, ErrorClass::Retryable);
        assert_eq!(err1.category, "NetworkService");
        assert!(err1.tip.unwrap().contains("remote server"));

        let err2 = classify_error("write_file", "Permission denied");
        assert_eq!(err2.class, ErrorClass::Recoverable);
        assert_eq!(err2.category, "PermissionDenied");
        assert!(err2.tip.unwrap().contains("permission issue"));

        let err3 = classify_error("read_file", "no such file or directory");
        assert_eq!(err3.class, ErrorClass::Fatal);
        assert_eq!(err3.category, "NotFound");
        assert!(err3.tip.unwrap().contains("does not exist"));
    }
}
