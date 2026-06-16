use ollama_rs::Ollama;
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;
use tokio::sync::mpsc;

use tempest_ai::checkpoint::CheckpointManager;
use tempest_ai::inference::Backend;
use tempest_ai::tools::rust::CrateSearchTool;
use tempest_ai::tools::{AgentTool, ToolContext};
use tempest_ai::vector_brain::VectorBrain;
use skg_tool::ToolDyn;

fn make_mock_context() -> (ToolContext, skg_tool::ToolCallContext) {
    let (tx, _) = mpsc::channel(10);
    let (_, tool_rx_inner) = mpsc::channel(10);
    let tool_rx = Arc::new(tokio::sync::Mutex::new(Some(tool_rx_inner)));

    let context = ToolContext {
        ollama: Ollama::default(),
        backend: Arc::new(RwLock::new(Backend::Ollama(Ollama::default()))),
        model: "test".to_string(),
        sub_agent_model: "test".to_string(),
        history: Arc::new(Mutex::new(vec![])),
        task_context: Arc::new(Mutex::new("test".to_string())),
        vector_brain: Arc::new(Mutex::new(VectorBrain::new())),
        telemetry: Arc::new(Mutex::new("test".to_string())),
        tx: Some(tx),
        tool_rx,
        recent_tool_calls: Arc::new(dashmap::DashMap::new()),
        brain_path: std::path::PathBuf::from("test"),
        is_root: Arc::new(AtomicBool::new(false)),
        all_tools: vec![],
        checkpoint_mgr: Arc::new(Mutex::new(CheckpointManager::new(10))),
        memory_store: Arc::new(Mutex::new(tempest_ai::memory::MemoryStore::new("test".to_string()).unwrap())),
    };

    let skg_ctx = skg_tool::ToolCallContext::with_deps(
        layer0::id::OperatorId::new("tempest-agent"),
        Arc::new(Arc::new(context.clone())),
    );

    (context, skg_ctx)
}

#[tokio::test]
async fn test_legacy_cargo_search_success() {
    let tool = CrateSearchTool;
    let (context, _) = make_mock_context();

    let args = json!({
        "query": "serde"
    });

    let res = tool.execute(&args, context).await.unwrap();
    assert!(res.contains("serde"));
}

#[tokio::test]
async fn test_native_cargo_search_success() {
    let tool = tempest_ai::tools::skg_tools::rust::CargoSearchTool::new();
    let (_, skg_ctx) = make_mock_context();

    let args = json!({
        "query": "serde"
    });

    let res = tool.call(args, &skg_ctx).await.unwrap();
    let res_str = res.as_str().unwrap();
    assert!(res_str.contains("serde"));
}

#[tokio::test]
async fn test_native_cargo_search_not_found() {
    let tool = tempest_ai::tools::skg_tools::rust::CargoSearchTool::new();
    let (_, skg_ctx) = make_mock_context();

    // Search for a highly unlikely randomized string
    let args = json!({
        "query": "nonexistent_crate_z92jfkd8"
    });

    let res = tool.call(args, &skg_ctx).await.unwrap();
    let res_str = res.as_str().unwrap();
    assert!(res_str.contains("No crates found matching your query") || res_str.contains("search_web"));
}
