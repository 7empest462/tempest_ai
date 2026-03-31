use std::fs;
use std::path::PathBuf;

/// Represents a single skill loaded from ~/.tempest/skills/
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub instructions: String,
}

/// Returns the path to the skills directory, creating it if needed.
pub fn skills_dir() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".tempest");
    path.push("skills");
    let _ = fs::create_dir_all(&path);
    path
}

/// Returns the path to the brain directory, creating it if needed.
pub fn brain_dir() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".tempest");
    path.push("brain");
    let _ = fs::create_dir_all(&path);
    path
}

/// Load all skills from ~/.tempest/skills/*.md
/// Each skill file has YAML frontmatter (name, description) and markdown body.
pub fn load_skills() -> Vec<Skill> {
    let dir = skills_dir();
    let mut skills = Vec::new();

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return skills,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Some(skill) = parse_skill_file(&content) {
                    skills.push(skill);
                }
            }
        }
    }

    skills
}

/// Parse a skill markdown file with YAML frontmatter
fn parse_skill_file(content: &str) -> Option<Skill> {
    // Expect format:
    // ---
    // name: skill_name
    // description: What this skill does
    // ---
    // ## Instructions
    // ...
    let trimmed = content.trim();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_first = &trimmed[3..];
    let end_marker = after_first.find("---")?;
    let frontmatter = &after_first[..end_marker];
    let body = after_first[end_marker + 3..].trim();

    let mut name = String::new();
    let mut description = String::new();

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("name:") {
            name = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("description:") {
            description = val.trim().to_string();
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(Skill {
        name,
        description,
        instructions: body.to_string(),
    })
}

/// Load all brain knowledge items from ~/.tempest/brain/*.md
/// Returns (topic, summary, created_date) for each item.
pub fn load_brain_items() -> Vec<(String, String, String)> {
    let dir = brain_dir();
    let mut items = Vec::new();

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return items,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Some(item) = parse_brain_file(&content) {
                    items.push(item);
                }
            }
        }
    }

    // Sort by date descending and cap at 10
    items.sort_by(|a, b| b.2.cmp(&a.2));
    items.truncate(10);
    items
}

/// Parse a brain knowledge item markdown file
fn parse_brain_file(content: &str) -> Option<(String, String, String)> {
    let trimmed = content.trim();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_first = &trimmed[3..];
    let end_marker = after_first.find("---")?;
    let frontmatter = &after_first[..end_marker];
    let body = after_first[end_marker + 3..].trim();

    let mut topic = String::new();
    let mut created = String::new();

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("topic:") {
            topic = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("created:") {
            created = val.trim().to_string();
        }
    }

    if topic.is_empty() {
        return None;
    }

    // Truncate summary to 500 chars to save context tokens
    let summary = if body.len() > 500 {
        let safe_len = body.char_indices().nth(500).map(|(i, _)| i).unwrap_or(body.len());
        format!("{}...", &body[..safe_len])
    } else {
        body.to_string()
    };

    Some((topic, summary, created))
}

/// Search brain items by keyword
pub fn search_brain(keyword: &str) -> Vec<(String, String)> {
    let dir = brain_dir();
    let mut results = Vec::new();
    let keyword_lower = keyword.to_lowercase();

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return results,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Some((topic, summary, _)) = parse_brain_file(&content) {
                    if topic.to_lowercase().contains(&keyword_lower) || summary.to_lowercase().contains(&keyword_lower) {
                        results.push((topic, summary));
                    }
                }
            }
        }
    }

    results
}
