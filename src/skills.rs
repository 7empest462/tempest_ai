use std::path::PathBuf;
use std::fs;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub instructions: String,
}

pub fn skills_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = home.join(".tempest").join("skills");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

pub fn brain_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = home.join(".tempest").join("brain");
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

pub fn load_skills() -> Vec<Skill> {
    let dir = skills_dir();
    let mut skills = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "md") {
                if let Ok(content) = fs::read_to_string(&path) {
                    // Very basic YAML frontmatter parser
                    if content.starts_with("---") {
                        let parts: Vec<&str> = content.split("---").collect();
                        if parts.len() >= 3 {
                            let yaml = parts[1];
                            let instructions = parts[2..].join("---").trim().to_string();
                            
                            let mut name = String::new();
                            let mut description = String::new();
                            
                            for line in yaml.lines() {
                                if line.starts_with("name:") {
                                    name = line["name:".len()..].trim().to_string();
                                } else if line.starts_with("description:") {
                                    description = line["description:".len()..].trim().to_string();
                                }
                            }
                            
                            if !name.is_empty() {
                                skills.push(Skill { name, description, instructions });
                            }
                        }
                    }
                }
            }
        }
    }
    skills
}

pub fn search_brain(keyword: &str) -> Vec<(String, String)> {
    let dir = brain_dir();
    let mut results = Vec::new();
    let keyword_lower = keyword.to_lowercase();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "md") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if content.to_lowercase().contains(&keyword_lower) {
                        // Extract topic from frontmatter
                        let mut topic = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                        for line in content.lines() {
                            if line.starts_with("topic:") {
                                topic = line["topic:".len()..].trim().to_string();
                                break;
                            }
                        }
                        
                        // Extract summary (everything after frontmatter)
                        let summary = if content.starts_with("---") {
                            let parts: Vec<&str> = content.split("---").collect();
                            if parts.len() >= 3 {
                                parts[2..].join("---").trim().to_string()
                            } else {
                                content.clone()
                            }
                        } else {
                            content.clone()
                        };
                        
                        results.push((topic, summary));
                    }
                }
            }
        }
    }
    results
}
