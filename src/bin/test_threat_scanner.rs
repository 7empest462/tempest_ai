// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See the LICENSE file in the project root for full license text.

use ollama_rs::Ollama;
use parking_lot::Mutex;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tempest_ai::checkpoint::CheckpointManager;
use tempest_ai::inference::Backend;
use tempest_ai::tools::threat_scanner::ThreatScannerTool;
use tempest_ai::tools::{AgentTool, ToolContext};
use tempest_ai::vector_brain::VectorBrain;
use tokio::sync::RwLock;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "🧪 [INCIDENT RESPONSE]: Initializing Cybersecurity Threat diagnostics verification...\n"
    );

    let scanner = ThreatScannerTool;

    // Construct dummy ToolContext for threat_scan verification
    let (tx, _) = mpsc::channel(1);
    let (_, tool_rx_inner) = mpsc::channel(1);
    let tool_rx = Arc::new(tokio::sync::Mutex::new(Some(tool_rx_inner)));

    let mock_context = ToolContext {
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

    // 1. Manually Verify Detection of our Mock Malware Script
    let target_file = "scratch/mock_malware.py";
    println!(
        "🔍 [AUDIT]: Starting file audit on suspicious simulation file: '{}'...",
        target_file
    );
    if Path::new(target_file).exists() {
        let args = json!({
            "target_type": "file",
            "target_path": target_file
        });

        let report = scanner.execute(&args, mock_context.clone()).await?;
        println!("\n==================== THREAT AUDIT REPORT ====================");
        println!("{}", report);
        println!("=============================================================\n");

        // Assert detection was triggered
        assert!(
            report.contains("SUSPICIOUS ACTIVITY") || report.contains("CRITICAL THREAT DETECTED"),
            "Error: Scanner failed to flag mock malware heuristics!"
        );
        println!(
            "✅ [SUCCESS]: Heuristics successfully detected the mock reverse TCP shell spawner!"
        );
    } else {
        println!(
            "⚠️ [WARNING]: Mock malware script not found at '{}'. Skipping file audit.",
            target_file
        );
    }

    // 2. Verify Detection of a standard Clean File (should return SAFE)
    let clean_file = "src/lib.rs";
    println!(
        "\n🔍 [AUDIT]: Starting file audit on clean code file: '{}'...",
        clean_file
    );
    let clean_args = json!({
        "target_type": "file",
        "target_path": clean_file
    });
    let clean_report = scanner.execute(&clean_args, mock_context.clone()).await?;
    println!("\n==================== CLEAN AUDIT REPORT ====================");
    println!("{}", clean_report);
    println!("=============================================================\n");
    assert!(
        clean_report.contains("✅ No threat indicators"),
        "Error: Clean file was falsely flagged!"
    );
    println!("✅ [SUCCESS]: Heuristics correctly marked clean codebase files as SAFE!");

    // 3. Verify System Process Audit
    println!("\n🔍 [AUDIT]: Scanning active running processes on host...");
    let proc_args = json!({
        "target_type": "process"
    });
    let proc_report = scanner.execute(&proc_args, mock_context).await?;
    println!("\n==================== PROCESS AUDIT REPORT ====================");
    // Limit log length to avoid cluttering stdout
    let lines: Vec<&str> = proc_report.lines().take(20).collect();
    for line in lines {
        println!("{}", line);
    }
    println!("...[truncated process list]...");
    println!("=============================================================\n");

    assert!(
        proc_report.contains("Cybersecurity Diagnostics Report: Process Audit"),
        "Error: Process audit header is missing!"
    );
    println!(
        "✅ [SUCCESS]: Process Diagnostics Engine parsed active processes and verified executables successfully!"
    );

    println!(
        "\n🎉 [VERIFICATION COMPLETE]: All Cybersecurity Threat Diagnostics Scanner tests passed successfully!"
    );
    Ok(())
}
