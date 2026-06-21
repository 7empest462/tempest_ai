use ollama_rs::Ollama;
use parking_lot::Mutex;
use serde_json::json;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tempest_ai::checkpoint::CheckpointManager;
use tempest_ai::inference::Backend;
use tempest_ai::tools::csv::QueryCsvTool;
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
async fn test_csv_tool_inspect_headers() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        "name,age,city\nAlice,30,New York\nBob,25,San Francisco"
    )
    .unwrap();
    let csv_path = file.path().to_str().unwrap().to_string();

    let tool = QueryCsvTool;
    let context = make_mock_context();

    let args = json!({
        "csv_path": csv_path,
        "action": "inspect_headers"
    });

    let res = tool.execute(&args, context).await.unwrap();
    assert!(res.contains("name"));
    assert!(res.contains("age"));
    assert!(res.contains("city"));
    assert!(res.contains("Index 0"));
}

#[tokio::test]
async fn test_csv_tool_get_rows() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        "name,age,city\nAlice,30,New York\nBob,25,San Francisco\nCharlie,35,Seattle"
    )
    .unwrap();
    let csv_path = file.path().to_str().unwrap().to_string();

    let tool = QueryCsvTool;
    let context = make_mock_context();

    let args = json!({
        "csv_path": csv_path,
        "action": "get_rows",
        "limit": 2,
        "offset": 0
    });

    let res = tool.execute(&args, context).await.unwrap();
    assert!(res.contains("Alice"));
    assert!(res.contains("Bob"));
    assert!(!res.contains("Charlie"));
}

#[tokio::test]
async fn test_csv_tool_filter_by_column() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        "name,age,city\nAlice,30,New York\nBob,25,San Francisco\nCharlie,35,New York"
    )
    .unwrap();
    let csv_path = file.path().to_str().unwrap().to_string();

    let tool = QueryCsvTool;
    let context = make_mock_context();

    let args = json!({
        "csv_path": csv_path,
        "action": "filter_by_column",
        "filter_column": "city",
        "filter_value": "new york"
    });

    let res = tool.execute(&args, context).await.unwrap();
    assert!(res.contains("Alice"));
    assert!(res.contains("Charlie"));
    assert!(!res.contains("Bob"));
}
