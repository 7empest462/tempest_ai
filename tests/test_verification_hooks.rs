use ollama_rs::generation::chat::MessageRole;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tempest_ai::agent::{Agent, AgentConfig, AgentStream, AgentStreamState};
use tempest_ai::inference::AgentMode;
use tempest_ai::turn_kit::VerificationHook;

#[tokio::test(flavor = "multi_thread")]
async fn test_verification_hook_execution_on_modify() {
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

    let mut stream = AgentStream::new(&agent, Arc::new(AtomicBool::new(false)));

    // Clear auto-detected hooks to ensure we only run the test hook and avoid concurrent cargo lock deadlocks
    stream.decomposer.hooks.clear();

    // Register a failing hook using standard 'ls' command targeting a nonexistent file.
    // This exists and executes successfully on Unix/macOS, returning exit code 1 or similar.
    stream.decomposer.register_hook(VerificationHook {
        name: "Test Failing Hook".to_string(),
        command: "ls nonexistent_file_xyz_123".to_string(),
    });

    // Create a safe temporary path
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!(
        "test_hook_{}.txt",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path_str = temp_path.to_str().unwrap().to_string();

    // Set state to PendingTools with a modifying tool call ("write_file")
    stream.state = AgentStreamState::PendingTools {
        tool_calls: vec![ollama_rs::generation::tools::ToolCall {
            function: ollama_rs::generation::tools::ToolCallFunction {
                name: "write_file".to_string(),
                arguments: serde_json::json!({
                    "path": path_str,
                    "content": "test new content",
                    "force_overwrite": true,
                }),
            },
        }],
    };

    // Transition the stream, which will execute the tool call, then trigger the verification hook
    let res = stream.transition().await;
    assert!(res.is_ok(), "Transition failed: {:?}", res);

    // After failure, it transitions back to thinking to allow the agent to correct it
    assert!(
        matches!(stream.state, AgentStreamState::Thinking { .. }),
        "Expected Thinking state, got {:?}",
        stream.state
    );

    // Verify system reprimand was injected into history
    let history = agent.history.lock();
    assert!(!history.is_empty(), "History should not be empty");
    let last_msg = history.last().unwrap();
    assert_eq!(last_msg.role, MessageRole::System);
    assert!(
        last_msg.content.contains("[VERIFICATION FAILED]"),
        "Expected reprimand, got: {}",
        last_msg.content
    );
    assert!(
        last_msg.content.contains("Test Failing Hook"),
        "Expected 'Test Failing Hook' failure in reprimand, got: {}",
        last_msg.content
    );

    // Clean up temporary file
    let _ = std::fs::remove_file(temp_path);
}
