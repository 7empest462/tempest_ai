use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use super::{AgentTool, ToolContext};

pub struct ListSkillsTool;

#[async_trait]
impl AgentTool for ListSkillsTool {
    fn name(&self) -> &'static str { "list_skills" }
    fn description(&self) -> &'static str { "List all available skills from ~/.tempest/skills/. Skills are reusable, step-by-step recipes for common tasks." }
    fn parameters(&self) -> Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }
    async fn execute(&self, _args: &Value, _context: ToolContext) -> Result<String> {
        let skills = crate::skills::load_skills();
        if skills.is_empty() {
            Ok("No skills found in ~/.tempest/skills/. Create a .md file with YAML frontmatter (name, description) and markdown instructions to add a skill.".to_string())
        } else {
            let mut out = format!("📋 {} skills available:\n", skills.len());
            for skill in &skills {
                out.push_str(&format!("  • {} — {}\n", skill.name, skill.description));
            }
            out.push_str("\nUse `recall_skill` with a skill name to see its full instructions.");
            Ok(out)
        }
    }
}

pub struct SkillRecallTool;

#[async_trait]
impl AgentTool for SkillRecallTool {
    fn name(&self) -> &'static str { "recall_skill" }
    fn description(&self) -> &'static str { "Recall the full instructions of a specific skill by name. Use this to follow a predefined recipe for a task." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "The name of the skill to recall (as listed by list_skills)." }
            },
            "required": ["name"]
        })
    }
    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let name = args.get("name").and_then(|n| n.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'name'"))?;
        let skills = crate::skills::load_skills();
        if let Some(skill) = skills.iter().find(|s| s.name == name) {
            Ok(format!("🔧 SKILL: {}\n{}\n\n--- INSTRUCTIONS ---\n{}", skill.name, skill.description, skill.instructions))
        } else {
            Ok(format!("No skill found with name '{}'. Use `list_skills` to see available skills.", name))
        }
    }
}

pub struct DistillKnowledgeTool;

#[async_trait]
impl AgentTool for DistillKnowledgeTool {
    fn name(&self) -> &'static str { "distill_knowledge" }
    fn description(&self) -> &'static str { "After completing a significant task, write a distilled 1-paragraph summary to your brain for future reference. This creates a persistent knowledge item in ~/.tempest/brain/." }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "topic": { "type": "string", "description": "A short slug identifying this knowledge (e.g., 'network_scanner', 'rust_procfs_fix')." },
                "summary": { "type": "string", "description": "A 1-2 paragraph summary of what you did, key decisions, gotchas, and user preferences you observed." }
            },
            "required": ["topic", "summary"]
        })
    }
    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let topic = args.get("topic").and_then(|t| t.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'topic'"))?;
        let summary = args.get("summary").and_then(|s| s.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'summary'"))?;

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

        std::fs::write(&filepath, &content)?;
        Ok(format!("🧠 Knowledge distilled! Saved '{}' to {}. This will be available in future sessions.", topic, filepath.display()))
    }
}

pub struct RecallBrainTool;

#[async_trait]
impl AgentTool for RecallBrainTool {
    fn name(&self) -> &'static str { "recall_brain" }
    fn description(&self) -> &'static str { "Search your brain directory for knowledge items related to a topic. Returns distilled summaries from previous sessions." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "keyword": { "type": "string", "description": "The keyword to search for in your brain knowledge items." }
            },
            "required": ["keyword"]
        })
    }
    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let keyword = args.get("keyword").and_then(|k| k.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'keyword'"))?;
        let results = crate::skills::search_brain(keyword);
        if results.is_empty() {
            Ok(format!("No brain knowledge items found matching '{}'. Use `distill_knowledge` after a significant task to build your knowledge base.", keyword))
        } else {
            let mut out = format!("🧠 Found {} knowledge items matching '{}':\n\n", results.len(), keyword);
            for (topic, summary) in &results {
                out.push_str(&format!("--- {} ---\n{}\n\n", topic, summary));
            }
            Ok(out)
        }
    }
}
