// ==========================================
// 🧠 SKG KNOWLEDGE TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy AgentTool knowledge tools.

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;

// ── list_skills ────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "list_skills",
    description = "List all available skills from ~/.tempest/skills/. Skills are reusable, step-by-step recipes for common tasks."
)]
pub async fn list_skills() -> Result<serde_json::Value, ToolError> {
    let skills = crate::skills::load_skills();
    if skills.is_empty() {
        Ok(serde_json::Value::String("No skills found in ~/.tempest/skills/. Create a .md file with YAML frontmatter (name, description) and markdown instructions to add a skill.".to_string()))
    } else {
        let mut out = format!("📋 {} skills available:\n", skills.len());
        for skill in &skills {
            out.push_str(&format!("  • {} — {}\n", skill.name, skill.description));
        }
        out.push_str("\nUse `recall_skill` with a skill name to see its full instructions.");
        Ok(serde_json::Value::String(out))
    }
}

// ── recall_skill ───────────────────────────────────────────────────────────────

#[skg_tool(
    name = "recall_skill",
    description = "Recall the full instructions of a specific skill by name. Use this to follow a predefined recipe for a task."
)]
pub async fn recall_skill(name: String) -> Result<serde_json::Value, ToolError> {
    let skills = crate::skills::load_skills();
    if let Some(skill) = skills.iter().find(|s| s.name == name) {
        Ok(serde_json::Value::String(format!(
            "🔧 SKILL: {}\n{}\n\n--- INSTRUCTIONS ---\n{}",
            skill.name, skill.description, skill.instructions
        )))
    } else {
        Ok(serde_json::Value::String(format!(
            "No skill found with name '{}'. Use `list_skills` to see available skills.",
            name
        )))
    }
}

// ── distill_knowledge ──────────────────────────────────────────────────────────

#[skg_tool(
    name = "distill_knowledge",
    description = "After completing a significant task, write a distilled 1-paragraph summary to your brain for future reference. This creates a persistent knowledge item in ~/.tempest/brain/."
)]
pub async fn distill_knowledge(
    topic: String,
    summary: String,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx
        .deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    let dir = crate::skills::brain_dir();
    let now = chrono::Local::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    let timestamp = now.format("%Y-%m-%dT%H:%M:%S%z").to_string();
    let filename = format!("{}_{}.md", date_str, topic.replace(' ', "_").to_lowercase());
    let filepath = dir.join(&filename);

    let content = format!(
        "---\ntopic: {}\ncreated: {}\n---\n{}",
        topic, timestamp, summary
    );

    std::fs::write(&filepath, content).map_err(|e| {
        ToolError::ExecutionFailed(format!("Failed to write knowledge item: {}", e))
    })?;

    // 🧠 SEMANTIC SYNC (Index the new knowledge item immediately)
    let backend = tool_ctx.backend.read().await;
    if let Ok(embedding) = backend.generate_embeddings(&summary).await {
        let mut brain = tool_ctx.vector_brain.lock();
        brain.add_entry(
            summary.clone(),
            embedding,
            format!("brain:{}", filename),
            std::collections::HashMap::new(),
        );
        let _ = brain.save_to_disk(&tool_ctx.brain_path);
    }

    Ok(serde_json::Value::String(format!(
        "🧠 Knowledge distilled! Saved '{}' to {}. This is now conceptually indexed in your neural memory.",
        topic,
        filepath.display()
    )))
}

// ── recall_brain ───────────────────────────────────────────────────────────────

#[skg_tool(
    name = "recall_brain",
    description = "Search your brain directory for knowledge items related to a topic. Returns distilled summaries from previous sessions."
)]
pub async fn recall_brain(
    keyword: String,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx
        .deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    if let Some(ref tx) = tool_ctx.tx {
        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
            "🧠 Searching neural memory for: '{}'...",
            keyword
        )));
    }

    let backend = tool_ctx.backend.read().await;
    let query_vector = backend
        .generate_embeddings(&keyword)
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Embedding generation failed: {}", e)))?;

    let brain = tool_ctx.vector_brain.lock();
    let results = brain.search(&query_vector, 5);

    if results.is_empty() {
        Ok(serde_json::Value::String(format!(
            "No brain knowledge items found matching the concept '{}'. Use `distill_knowledge` after a significant task to build your knowledge base.",
            keyword
        )))
    } else {
        let mut out = format!(
            "🧠 Found {} relevant knowledge items for '{}':\n\n",
            results.len(),
            keyword
        );
        for (entry, score) in results {
            out.push_str(&format!(
                "--- {} (Confidence: {:.1}%) ---\n{}\n\n",
                entry.source,
                score * 100.0,
                entry.text
            ));
        }
        Ok(serde_json::Value::String(out))
    }
}
