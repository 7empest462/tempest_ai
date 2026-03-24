#![allow(dead_code)]

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TempestError {
    #[error("Failed to connect to Ollama: {0}")]
    OllamaConnection(String),

    #[error("Tool execution failed: {tool} — {message}")]
    ToolExecution { tool: String, message: String },

    #[error("Configuration parse error: {0}")]
    ConfigParse(String),

    #[error("History file corrupted or unreadable: {0}")]
    HistoryCorrupted(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Session error: {0}")]
    Session(String),
}
