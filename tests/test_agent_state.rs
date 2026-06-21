use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use parking_lot::Mutex;
use std::sync::Arc;
use tempest_ai::agent::{Agent, AgentConfig};
use tempest_ai::inference::AgentMode;

#[tokio::test(flavor = "multi_thread")]
async fn test_in_memory_state_store() {
    let memory_store = Arc::new(Mutex::new(
        tempest_ai::memory::MemoryStore::new("test-passphrase".to_string()).unwrap(),
    ));

    // Create agent with ":memory:" path
    let agent = Agent::new(
        AgentMode::Ollama,
        "test-model".to_string(),
        "Q4_K_M".to_string(),
        "test-prompt".to_string(),
        ":memory:".to_string(),
        "test-session-id".to_string(),
        memory_store,
        "test-sub-model".to_string(),
        "test-embedding-model".to_string(),
        Arc::new(Mutex::new(None)),
        AgentConfig {
            planner_model: None,
            executor_model: None,
            verifier_model: None,
            mlx_presets: std::collections::HashMap::new(),
            temp_planning: 0.05,
            temp_execution: 0.25,
            top_p_planning: 0.95,
            top_p_execution: 0.92,
            repeat_penalty_planning: 1.18,
            repeat_penalty_execution: 1.12,
            ctx_planning: 16384,
            ctx_execution: 32768,
            mlx_temp_planning: None,
            mlx_temp_execution: None,
            mlx_top_p_planning: None,
            mlx_top_p_execution: None,
            mlx_repeat_penalty_planning: None,
            mlx_repeat_penalty_execution: None,
            paged_attn: false,
            planning_enabled: true,
            lmstudio_url: None,
            pa_memory_mb: None,
            vram_time_sharing: false,
            ollama_remote: None,
            tool_engine: "legacy".to_string(),
        },
    )
    .await
    .unwrap();

    // Verify state store is memory state store (we can test indirectly by saving/loading history)
    {
        let mut history = agent.history.lock();
        history.push(ChatMessage::new(
            MessageRole::User,
            "Hello in-memory store".to_string(),
        ));
    }

    // Save history
    agent.save_history().unwrap();

    // Clear history in memory
    {
        let mut history = agent.history.lock();
        history.clear();
    }

    // Load history
    agent.load_history().unwrap();

    // Assert that the history was successfully restored from our MemoryStateStore
    {
        let history = agent.history.lock();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].content, "Hello in-memory store");
    }
}
