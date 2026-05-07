use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tempest_ai::tools::file::ReadFileTool;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;
use tempest_ai::tools::{ToolContext, AgentTool};
use ollama_rs::Ollama;
use std::sync::atomic::AtomicBool;
use parking_lot::Mutex;
use tempest_ai::vector_brain::VectorBrain;
use tempest_ai::checkpoint::CheckpointManager;
use tempest_ai::inference::Backend;
use tokio::sync::RwLock;

fn bench_read_file_tool(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("read_file_tool", |b| {
        b.to_async(&runtime).iter(|| async {
            // Create a temporary file for testing
            let temp_file = tempfile::NamedTempFile::new().unwrap();
            std::fs::write(&temp_file, "Hello, world!").unwrap();

            let args = json!({
                "path": temp_file.path().to_str().unwrap()
            });

            // Mock context - simplified for benchmarking
            let (tx, _) = mpsc::channel(1);
            let (_, tool_rx_inner) = mpsc::channel(1);
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
            };

            let tool = ReadFileTool;
            let result = tool.execute(&args, context).await.expect("Tool execution failed during benchmark");
            black_box(result);
        })
    });
}

criterion_group!(benches, bench_read_file_tool);
criterion_main!(benches);