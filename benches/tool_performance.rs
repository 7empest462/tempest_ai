use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use ollama_rs::Ollama;
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tempest_ai::checkpoint::CheckpointManager;
use tempest_ai::inference::Backend;
use tempest_ai::tools::{AgentTool, ToolContext};
use tempest_ai::vector_brain::VectorBrain;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use skg_tool::ToolDyn;

// Imports
use tempest_ai::tools::file::ReadFileTool as LegacyReadFileTool;
use tempest_ai::tools::skg_tools::file::ReadFileTool as SkgReadFileTool;

use tempest_ai::tools::wasm_sandbox::WasmSafeCalculatorTool as LegacyWasmSafeCalcTool;
use tempest_ai::tools::skg_tools::wasm_sandbox::WasmSafeCalcTool as SkgWasmSafeCalcTool;

use tempest_ai::tools::csv::QueryCsvTool as LegacyQueryCsvTool;
use tempest_ai::tools::skg_tools::csv::QueryCsvTool as SkgQueryCsvTool;

fn create_mock_context() -> (ToolContext, skg_tool::ToolCallContext) {
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
        memory_store: Arc::new(Mutex::new(tempest_ai::memory::MemoryStore::new("test".to_string()).unwrap())),
    };

    let skg_ctx = skg_tool::ToolCallContext::with_deps(
        layer0::id::OperatorId::new("tempest-agent"),
        Arc::new(Arc::new(context.clone())),
    );

    (context, skg_ctx)
}

fn bench_read_file_comparison(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Create a temporary file for testing
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(&temp_file, "Hello, world! Benchmark comparison payload!").unwrap();
    let file_path = temp_file.path().to_str().unwrap().to_string();

    c.bench_function("legacy_read_file", |b| {
        b.to_async(&runtime).iter(|| async {
            let (context, _) = create_mock_context();
            let args = json!({ "path": file_path });
            let tool = LegacyReadFileTool;
            let result = tool.execute(&args, context).await.unwrap();
            black_box(result);
        })
    });

    c.bench_function("skg_read_file", |b| {
        b.to_async(&runtime).iter(|| async {
            let (_, skg_ctx) = create_mock_context();
            let args = json!({ "path": file_path });
            let tool = SkgReadFileTool::new();
            let result = tool.call(args, &skg_ctx).await.unwrap();
            black_box(result);
        })
    });
}

fn bench_wasm_safe_calc_comparison(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("legacy_wasm_safe_calc", |b| {
        b.to_async(&runtime).iter(|| async {
            let (context, _) = create_mock_context();
            let args = json!({
                "lh": 42,
                "rh": 10,
                "op": "add"
            });
            let tool = LegacyWasmSafeCalcTool;
            let result = tool.execute(&args, context).await.unwrap();
            black_box(result);
        })
    });

    c.bench_function("skg_wasm_safe_calc", |b| {
        b.to_async(&runtime).iter(|| async {
            let (_, skg_ctx) = create_mock_context();
            let args = json!({
                "lh": 42,
                "rh": 10,
                "op": "add"
            });
            let tool = SkgWasmSafeCalcTool::new();
            let result = tool.call(args, &skg_ctx).await.unwrap();
            black_box(result);
        })
    });
}

fn bench_query_csv_comparison(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Create a temporary CSV file
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(&temp_file, "id,name,role\n1,Alice,Engineer\n2,Bob,Product").unwrap();
    let csv_path = temp_file.path().to_str().unwrap().to_string();

    c.bench_function("legacy_query_csv", |b| {
        b.to_async(&runtime).iter(|| async {
            let (context, _) = create_mock_context();
            let args = json!({
                "csv_path": csv_path,
                "action": "inspect_headers"
            });
            let tool = LegacyQueryCsvTool;
            let result = tool.execute(&args, context).await.unwrap();
            black_box(result);
        })
    });

    c.bench_function("skg_query_csv", |b| {
        b.to_async(&runtime).iter(|| async {
            let (_, skg_ctx) = create_mock_context();
            let args = json!({
                "csv_path": csv_path,
                "action": "inspect_headers"
            });
            let tool = SkgQueryCsvTool::new();
            let result = tool.call(args, &skg_ctx).await.unwrap();
            black_box(result);
        })
    });
}

criterion_group!(
    benches,
    bench_read_file_comparison,
    bench_wasm_safe_calc_comparison,
    bench_query_csv_comparison
);
criterion_main!(benches);
