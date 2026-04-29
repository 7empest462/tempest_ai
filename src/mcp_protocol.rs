// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See LICENSE in the project root for full license information.

// src/mcp_protocol.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum TempestRequest {
    #[serde(rename = "tempest/chat")]
    Chat {
        message: String,
        mode: Option<String>,
        backend: Option<String>,
        editor_context: Option<Value>,
    },

    #[serde(rename = "tempest/status")]
    Status,

    #[serde(rename = "tempest/switch_backend")]
    SwitchBackend {
        backend: String,
    },

    #[serde(rename = "tempest/clear_history")]
    ClearHistory,

    #[serde(rename = "tempest/get_state")]
    GetState,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatPayload {
    pub content: String,
    pub reasoning: Option<String>,
    pub is_streaming: bool,
    pub done: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum TempestResponse {
    #[serde(rename = "tempest/chat")]
    ChatResponse {
        #[serde(rename = "payload")]
        payload: ChatPayload,
    },

    #[serde(rename = "tempest/status")]
    StatusResponse {
        backend: String,
        phase: String,
        model: String,
        ram_usage_mb: u64,
        context_tokens: u64,
    },

    #[serde(rename = "tempest/switch_backend")]
    SwitchBackendResponse {
        success: bool,
        message: String,
    },

    #[serde(rename = "tempest/clear_history")]
    ClearHistoryResponse {
        success: bool,
    },

    #[serde(rename = "tempest/get_state")]
    StateResponse {
        phase: String,
        planning_enabled: bool,
        recent_tool_calls: Vec<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(flatten)]
    pub payload: TempestRequest,
}
