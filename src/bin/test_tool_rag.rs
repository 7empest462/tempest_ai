use tempest_ai::tools::{AgentTool, ToolContext};
use tempest_ai::tool_rag::{ToolVectorIndex, cosine_similarity, categorize_tool, ALWAYS_ON_TOOLS};
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use serde_json::Value;
use std::sync::Arc;
use miette::Result;

#[derive(serde::Deserialize, schemars::JsonSchema)]
struct DummyArgs {}

struct MockTool {
    name: &'static str,
    desc: &'static str,
}

#[async_trait::async_trait]
impl AgentTool for MockTool {
    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        self.desc
    }

    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<DummyArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            },
        }
    }

    async fn execute(&self, _args: &Value, _context: ToolContext) -> Result<String> {
        Ok("mocked".to_string())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 [TEST]: Starting Tool RAG & Vector similarity verification tests...");

    // 1. Verify ALWAYS_ON_TOOLS List Integrity
    println!("🔍 Testing ALWAYS_ON_TOOLS integrity...");
    assert!(ALWAYS_ON_TOOLS.contains(&"read_file"));
    assert!(ALWAYS_ON_TOOLS.contains(&"write_file"));
    assert!(ALWAYS_ON_TOOLS.contains(&"run_command"));
    assert!(ALWAYS_ON_TOOLS.contains(&"ask_user"));
    assert!(ALWAYS_ON_TOOLS.contains(&"query_schema"));
    println!("✅ ALWAYS_ON_TOOLS integrity passed!");

    // 2. Verify Cosine Similarity Mathematics
    println!("📐 Testing cosine similarity logic...");
    
    // Identical vectors -> similarity of 1.0
    let v1 = vec![1.0, 0.0, 0.0];
    let sim_ident = cosine_similarity(&v1, &v1);
    assert!((sim_ident - 1.0).abs() < 1e-5, "Identical vectors similarity should be 1.0, got {}", sim_ident);

    // Orthogonal vectors -> similarity of 0.0
    let v2 = vec![0.0, 1.0, 0.0];
    let sim_ortho = cosine_similarity(&v1, &v2);
    assert!(sim_ortho.abs() < 1e-5, "Orthogonal vectors similarity should be 0.0, got {}", sim_ortho);

    // Opposite vectors -> similarity of -1.0
    let v3 = vec![-1.0, 0.0, 0.0];
    let sim_opp = cosine_similarity(&v1, &v3);
    assert!((sim_opp - (-1.0)).abs() < 1e-5, "Opposite vectors similarity should be -1.0, got {}", sim_opp);

    // Non-trivial vectors
    let v4 = vec![1.0, 2.0, 3.0];
    let v5 = vec![4.0, 5.0, 6.0];
    let sim_val = cosine_similarity(&v4, &v5);
    // dot = 4 + 10 + 18 = 32
    // norm4 = sqrt(1 + 4 + 9) = sqrt(14) ≈ 3.741657
    // norm5 = sqrt(16 + 25 + 36) = sqrt(77) ≈ 8.774964
    // sim = 32 / (3.741657 * 8.774964) ≈ 32 / 32.83291 ≈ 0.97463
    assert!((sim_val - 0.9746318).abs() < 1e-5, "Math similarity check failed, got {}", sim_val);

    // Error/edge cases
    assert_eq!(cosine_similarity(&[], &[]), 0.0);
    assert_eq!(cosine_similarity(&[1.0], &[]), 0.0);
    assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
    println!("✅ Cosine similarity checks passed!");

    // 3. Verify Tool Categorization logic
    println!("🏷️ Testing tool categorization patterns...");
    assert_eq!(categorize_tool("git_status"), "git");
    assert_eq!(categorize_tool("git_action"), "git");
    assert_eq!(categorize_tool("read_file"), "filesystem");
    assert_eq!(categorize_tool("write_file"), "filesystem");
    assert_eq!(categorize_tool("search_web"), "web");
    assert_eq!(categorize_tool("run_command"), "execution");
    assert_eq!(categorize_tool("cargo_check"), "rust");
    assert_eq!(categorize_tool("skg_demo"), "skelegent");
    assert_eq!(categorize_tool("query_schema"), "general");
    println!("✅ Tool categorization checks passed!");

    // 4. Verify Fallback Index Build Behavior
    println!("🛡️ Testing ToolVectorIndex fallback initialization...");
    let mock_tools: Vec<Arc<dyn AgentTool>> = vec![
        Arc::new(MockTool { name: "read_file", desc: "Read file contents" }),
        Arc::new(MockTool { name: "write_file", desc: "Write file contents" }),
        Arc::new(MockTool { name: "git_status", desc: "Show git status" }),
        Arc::new(MockTool { name: "search_web", desc: "Search the web" }),
    ];

    let index = ToolVectorIndex::build_fallback(&mock_tools);
    assert_eq!(index.len(), 0, "Fallback index should have 0 embedded entries");
    assert_eq!(index.all_tools().len(), 4, "Should retain all 4 tools in all_tools metadata");

    // Verify resolve behavior (can be Ok if Ollama is online, or Err if offline/unavailable)
    let offline_backend = tempest_ai::inference::Backend::Ollama(ollama_rs::Ollama::default());
    let resolve_res = index.resolve("Find some files", &offline_backend, None).await;
    match resolve_res {
        Ok((selected_tools, _log)) => {
            println!("ℹ️ Ollama is online/reachable. Resolved {} tools.", selected_tools.len());
            assert!(!selected_tools.is_empty(), "Resolved tools list should not be empty");
        }
        Err(e) => {
            println!("ℹ️ Ollama is offline/unavailable (expected on clean environments): {}", e);
        }
    }
    println!("✅ ToolVectorIndex fallback initialization & resolution checks passed!");

    println!("🎉 [TEST SUCCESS]: All Tool RAG integration tests passed!");
    Ok(())
}
