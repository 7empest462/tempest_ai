#[derive(Debug, PartialEq, Eq)]
pub enum ErrorClass {
    /// Transient failure, worth retrying automatically (e.g. network timeout).
    Retryable,
    /// System-level barrier that might be overcome with a different strategy (e.g. sudo).
    Recoverable,
    /// Logical failure or missing resource, model must handle (e.g. file not found).
    Fatal,
}

/// Classifies a tool error based on the tool name and the error message.
pub fn classify_error(_tool_name: &str, error_msg: &str) -> ErrorClass {
    let msg = error_msg.to_lowercase();

    // 🌐 Network / IO Transient Failures
    if msg.contains("timed out") || 
       msg.contains("timeout") || 
       msg.contains("connection refused") || 
       msg.contains("reset by peer") ||
       msg.contains("error 400") || // Bad Request
       msg.contains("error 429") || // Too Many Requests
       msg.contains("error 500") || // Internal Server Error
       msg.contains("error 503") || // Service Unavailable
       msg.contains("error 502") || // Bad Gateway
       msg.contains("error 504") || // Gateway Timeout
       msg.contains("temporary failure in name resolution") || // DNS
       msg.contains("network is unreachable") ||
       msg.contains("broken pipe") ||
       msg.contains("connection reset")
    {
        return ErrorClass::Retryable;
    }

    // 🔒 Privilege / Permission Failures
    if msg.contains("permission denied") || 
       msg.contains("eacces") || 
       msg.contains("operation not permitted") ||
       msg.contains("sudo: a password is required")
    {
        return ErrorClass::Recoverable;
    }

    // 📂 Filesystem missing resources (usually Fatal since we expect the agent to check first)
    if msg.contains("no such file") || msg.contains("not found") || msg.contains("enoent") {
        return ErrorClass::Fatal;
    }

    // Default to Fatal to avoid infinite retry loops on unknown errors
    ErrorClass::Fatal
}
