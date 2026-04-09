use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tempest_ai::tools::file::ReadFileTool;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;
use tempest_ai::tools::ToolContext;
use ollama_rs::Ollama;
use std::collections::VecDeque;
use std::sync::Mutex;
use tempest_ai::vector_brain::VectorBrain;
use tempest_ai::memory::MemoryStore;

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
            let (_, tool_rx) = mpsc::channel(1);
            let tool_rx = Arc::new(tokio::sync::Mutex::new(tool_rx));

            let context = ToolContext {
                ollama: Ollama::default(),
                model: "test".to_string(),
                sub_agent_model: "test".to_string(),
                history: Arc::new(Mutex::new(vec![])),
                planning_mode: Arc::new(Mutex::new(false)),
                task_context: Arc::new(Mutex::new("test".to_string())),
                vector_brain: Arc::new(Mutex::new(VectorBrain::new())),
                telemetry: Arc::new(Mutex::new("test".to_string())),
                tx,
                tool_rx,
                recent_tool_calls: Arc::new(Mutex::new(VecDeque::new())),
                brain_path: std::path::PathBuf::from("test"),
                is_root: false,
            };

            let tool = ReadFileTool;
            let result = tool.execute(&args, context).await;
            black_box(result);
        })
    });
}

criterion_group!(benches, bench_read_file_tool);
criterion_main!(benches);