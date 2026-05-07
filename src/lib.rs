pub mod agent;
pub mod crypto;
pub mod error;
pub mod memory;
pub mod tools;
pub mod hardware;
pub mod daemon;
pub mod tui;
pub mod vector_brain;
pub mod skills;
pub mod context_manager;
pub mod error_classifier;
pub mod rules;
pub mod sentinel;
pub mod inference;
pub mod prompts;
pub mod checkpoint;
pub mod mcp;
pub mod mcp_server;
pub mod mcp_protocol;
pub mod ai_bridge;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MlxPreset {
    pub repo: Option<String>,
    pub path: Option<String>,
    pub quant: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
}

#[allow(dead_code)]
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AppConfig {
    pub model: Option<String>,
    pub history_path: Option<String>,
    pub db_path: Option<String>,
    pub encrypt_history: Option<bool>,
    pub sub_agent_model: Option<String>,
    pub mlx_model: Option<String>,
    pub mlx_quant: Option<String>,
    pub paged_attn: Option<bool>,
    pub planner_model: Option<String>,
    pub executor_model: Option<String>,
    pub verifier_model: Option<String>,
    pub lmstudio_model: Option<String>,
    pub lmstudio_url: Option<String>,
    pub mcp_servers: Option<Vec<McpServerConfig>>,
    pub mlx_presets: Option<HashMap<String, MlxPreset>>,
    pub temp_planning: Option<f32>,
    pub temp_execution: Option<f32>,
    pub top_p_planning: Option<f32>,
    pub top_p_execution: Option<f32>,
    pub repeat_penalty_planning: Option<f32>,
    pub repeat_penalty_execution: Option<f32>,
    pub ctx_planning: Option<u64>,
    pub ctx_execution: Option<u64>,
    pub mlx_temp_planning: Option<f32>,
    pub mlx_temp_execution: Option<f32>,
    pub mlx_top_p_planning: Option<f32>,
    pub mlx_top_p_execution: Option<f32>,
    pub mlx_repeat_penalty_planning: Option<f32>,
    pub mlx_repeat_penalty_execution: Option<f32>,
    pub planning_enabled: Option<bool>,
    pub tui_theme: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut mlx_presets = HashMap::new();
        mlx_presets.insert("r1".to_string(), MlxPreset {
            repo: Some("bartowski/DeepSeek-R1-Distill-Qwen-7B-GGUF".to_string()),
            quant: Some("Q8_0".to_string()),
            path: None,
            description: Some("DeepSeek R1 Distill Qwen 7B (GGUF)".to_string()),
        });
        mlx_presets.insert("qwen_big".to_string(), MlxPreset {
            repo: Some("bartowski/Qwen2.5-Coder-7B-Instruct-GGUF".to_string()),
            quant: Some("Q8_0".to_string()),
            path: None,
            description: Some("Qwen 2.5 Coder 7B Instruct (GGUF)".to_string()),
        });
        mlx_presets.insert("qwen_small".to_string(), MlxPreset {
            repo: Some("bartowski/Qwen2.5-Coder-7B-Instruct-GGUF".to_string()),
            quant: Some("Q4_K_M".to_string()),
            path: None,
            description: Some("Qwen 2.5 Coder 7B Instruct (Q4 GGUF)".to_string()),
        });

        AppConfig {
            model: Some("qwen2.5-coder:7b".to_string()),
            history_path: Some("history.json".to_string()),
            db_path: Some("~/fleet.db".to_string()),
            encrypt_history: Some(false),
            sub_agent_model: Some("llama3.2:1b".to_string()),
            mlx_model: Some("/Volumes/Corsair_Lab/Home/mlx_models/Tempest-Centurion-v8-Fused".to_string()),
            mlx_quant: Some("None".to_string()),
            paged_attn: None,
            planner_model: Some("deepseek-r1:8b".to_string()),
            executor_model: Some("qwen2.5-coder:7b".to_string()),
            verifier_model: Some("deepseek-r1:8b".to_string()),
            lmstudio_model: Some("Qwen3.5:9B".to_string()),
            lmstudio_url: Some("http://127.0.0.1:1234/v1".to_string()),
            mlx_presets: Some(mlx_presets),
            temp_planning: Some(0.6),
            temp_execution: Some(0.25),
            top_p_planning: Some(0.95),
            top_p_execution: Some(0.92),
            repeat_penalty_planning: Some(1.18),
            repeat_penalty_execution: Some(1.12),
            ctx_planning: Some(16384),
            ctx_execution: Some(16384),
            mlx_temp_planning: Some(0.6),
            mlx_temp_execution: Some(0.2),
            mlx_top_p_planning: Some(0.95),
            mlx_top_p_execution: Some(0.9),
            mlx_repeat_penalty_planning: Some(1.05),
            mlx_repeat_penalty_execution: Some(1.02),
            planning_enabled: Some(true),
            tui_theme: Some("base16-ocean.dark".to_string()),
            mcp_servers: None,
        }
    }
}
