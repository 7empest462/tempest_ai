use ollama_rs::Ollama;
use parking_lot::Mutex;
use serde_json::json;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tempest_ai::checkpoint::CheckpointManager;
use tempest_ai::inference::Backend;
use tempest_ai::tools::file::WriteFileTool;
use tempest_ai::tools::{AgentTool, ToolContext};
use tempest_ai::vector_brain::VectorBrain;
use tempfile::NamedTempFile;
use tokio::sync::RwLock;
use tokio::sync::mpsc;

fn make_mock_context() -> ToolContext {
    let (tx, _) = mpsc::channel(1);
    let (_, tool_rx_inner) = mpsc::channel(1);
    let tool_rx = Arc::new(tokio::sync::Mutex::new(Some(tool_rx_inner)));

    ToolContext {
        ollama: Ollama::default(),
        backend: Arc::new(RwLock::new(Backend::Ollama(
            Ollama::default(),
            "mxbai-embed-large".to_string(),
        ))),
        model: "test".to_string(),
        sub_agent_model: "test".to_string(),
        embedding_model: "mxbai-embed-large".to_string(),
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
        memory_store: Arc::new(Mutex::new(
            tempest_ai::memory::MemoryStore::new("test".to_string()).unwrap(),
        )),
    }
}

#[tokio::test]
async fn test_write_file_unescaping() {
    let file = NamedTempFile::new().unwrap();
    let file_path = file.path().to_str().unwrap().to_string();

    let tool = WriteFileTool;
    let context = make_mock_context();

    let args = json!({
        "path": file_path,
        "content": "[package]\\nname = \"speed_test_crust\"\\nedition = \"2021\"",
        "force_overwrite": true
    });

    let res = tool.execute(&args, context).await.unwrap();
    assert!(res.contains("Successfully wrote"));

    let content_written = fs::read_to_string(&file_path).unwrap();
    // Verify that the file content was properly unescaped to actual newlines
    assert_eq!(
        content_written,
        "[package]\nname = \"speed_test_crust\"\nedition = \"2021\""
    );
}

#[tokio::test]
async fn test_write_file_no_unescaping_when_newlines_exist() {
    let file = NamedTempFile::new().unwrap();
    let file_path = file.path().to_str().unwrap().to_string();

    let tool = WriteFileTool;
    let context = make_mock_context();

    // Contains actual newlines, so literal \n should NOT be unescaped
    let args = json!({
        "path": file_path,
        "content": "[package]\nname = \"speed_test_crust\"\\nedition = \"2021\"",
        "force_overwrite": true
    });

    let _res = tool.execute(&args, context).await.unwrap();
    let content_written = fs::read_to_string(&file_path).unwrap();
    assert_eq!(
        content_written,
        "[package]\nname = \"speed_test_crust\"\\nedition = \"2021\""
    );
}
