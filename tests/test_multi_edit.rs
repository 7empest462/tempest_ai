use ollama_rs::Ollama;
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tempfile::NamedTempFile;
use tokio::sync::RwLock;
use tokio::sync::mpsc;

use skg_tool::ToolDyn;
use tempest_ai::checkpoint::CheckpointManager;
use tempest_ai::inference::Backend;
use tempest_ai::tools::editing::MultiEditTool;
use tempest_ai::tools::{AgentTool, ToolContext};
use tempest_ai::vector_brain::VectorBrain;

fn make_mock_context() -> (ToolContext, skg_tool::ToolCallContext) {
    let (tx, _) = mpsc::channel(10);
    let (_, tool_rx_inner) = mpsc::channel(10);
    let tool_rx = Arc::new(tokio::sync::Mutex::new(Some(tool_rx_inner)));

    let context = ToolContext {
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
    };

    let skg_ctx = skg_tool::ToolCallContext::with_deps(
        layer0::id::OperatorId::new("tempest-agent"),
        Arc::new(Arc::new(context.clone())),
    );

    (context, skg_ctx)
}

#[tokio::test]
async fn test_legacy_multi_edit_success() {
    let file = NamedTempFile::new().unwrap();
    let original_content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
    std::fs::write(file.path(), original_content).unwrap();
    let path_str = file.path().to_str().unwrap().to_string();

    let tool = MultiEditTool;
    let (context, _) = make_mock_context();

    let args = json!({
        "path": path_str,
        "edits": [
            {
                "start_line": 1,
                "end_line": 2,
                "target": "line 2",
                "replacement": "line 2 modified"
            },
            {
                "start_line": 4,
                "end_line": 5,
                "target": "line 4",
                "replacement": "line 4 modified"
            }
        ]
    });

    let res = tool.execute(&args, context).await.unwrap();
    assert!(res.contains("Successfully applied"));

    let new_content = std::fs::read_to_string(file.path()).unwrap();
    let expected = "line 1\nline 2 modified\nline 3\nline 4 modified\nline 5\n";
    assert_eq!(new_content, expected);
}

#[tokio::test]
async fn test_native_multi_edit_success() {
    let file = NamedTempFile::new().unwrap();
    let original_content = "rust rules\njava drools\npython cool\n";
    std::fs::write(file.path(), original_content).unwrap();
    let path_str = file.path().to_str().unwrap().to_string();

    let tool = tempest_ai::tools::skg_tools::editing::MultiEditTool::new();
    let (_, skg_ctx) = make_mock_context();

    let args = json!({
        "path": path_str,
        "edits": [
            {
                "target": "rust rules",
                "replacement": "Rust is fast"
            },
            {
                "target": "python cool",
                "replacement": "Python is simple"
            }
        ]
    });

    let res = tool.call(args, &skg_ctx).await.unwrap();
    assert!(res.as_str().unwrap().contains("Successfully applied"));

    let new_content = std::fs::read_to_string(file.path()).unwrap();
    let expected = "Rust is fast\njava drools\nPython is simple\n";
    assert_eq!(new_content, expected);
}

#[tokio::test]
async fn test_multi_edit_target_not_found() {
    let file = NamedTempFile::new().unwrap();
    let original_content = "alpha\nbeta\ngamma\n";
    std::fs::write(file.path(), original_content).unwrap();
    let path_str = file.path().to_str().unwrap().to_string();

    let tool = MultiEditTool;
    let (context, _) = make_mock_context();

    let args = json!({
        "path": path_str,
        "edits": [
            {
                "target": "nonexistent",
                "replacement": "does not matter"
            }
        ]
    });

    let res = tool.execute(&args, context).await;
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("Target not found"));
}

#[tokio::test]
async fn test_multi_edit_ambiguous_target() {
    let file = NamedTempFile::new().unwrap();
    let original_content = "match\nmatch\nmatch\n";
    std::fs::write(file.path(), original_content).unwrap();
    let path_str = file.path().to_str().unwrap().to_string();

    let tool = MultiEditTool;
    let (context, _) = make_mock_context();

    // Ambiguous globally
    let args = json!({
        "path": path_str,
        "edits": [
            {
                "target": "match",
                "replacement": "replaced"
            }
        ]
    });

    let res = tool.execute(&args, context.clone()).await;
    assert!(res.is_err());
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("Multiple occurrences")
    );

    // Narrowed down to a single line -> should succeed!
    let args_narrowed = json!({
        "path": path_str,
        "edits": [
            {
                "start_line": 2,
                "end_line": 2,
                "target": "match",
                "replacement": "replaced"
            }
        ]
    });

    let res_narrowed = tool.execute(&args_narrowed, context).await.unwrap();
    assert!(res_narrowed.contains("Successfully applied"));

    let new_content = std::fs::read_to_string(file.path()).unwrap();
    assert_eq!(new_content, "match\nreplaced\nmatch\n");
}

#[tokio::test]
async fn test_multi_edit_overlapping_rejection() {
    let file = NamedTempFile::new().unwrap();
    let original_content = "abcdefgh\n";
    std::fs::write(file.path(), original_content).unwrap();
    let path_str = file.path().to_str().unwrap().to_string();

    let tool = MultiEditTool;
    let (context, _) = make_mock_context();

    let args = json!({
        "path": path_str,
        "edits": [
            {
                "target": "cde",
                "replacement": "X"
            },
            {
                "target": "def",
                "replacement": "Y"
            }
        ]
    });

    let res = tool.execute(&args, context).await;
    assert!(res.is_err());
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("Overlapping edits detected")
    );
}

#[tokio::test]
async fn test_multi_edit_approval_preview() {
    let file = NamedTempFile::new().unwrap();
    let original_content = "line 1\nline 2\n";
    std::fs::write(file.path(), original_content).unwrap();
    let path_str = file.path().to_str().unwrap().to_string();

    let tool = MultiEditTool;

    let args = json!({
        "path": path_str,
        "edits": [
            {
                "target": "line 2",
                "replacement": "line 2 updated"
            }
        ]
    });

    let preview = tool.get_approval_preview(&args).await.unwrap();
    assert!(preview.contains("Proposed multi-edit changes"));
    assert!(preview.contains("- line 2"));
    assert!(preview.contains("+ line 2 updated"));
}
