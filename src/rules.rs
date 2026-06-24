use miette::{IntoDiagnostic, Result};
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Rule {
    pub name: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    pub globs: Option<Vec<String>>,
    pub always_apply: Option<bool>,
    pub content: String,
}

#[derive(Clone)]
pub struct RuleEngine {
    pub rules: Vec<Rule>,
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleEngine {
    pub fn new() -> Self {
        let mut engine = RuleEngine { rules: Vec::new() };
        let _ = engine.load_all();
        engine
    }

    pub fn load_all(&mut self) -> Result<()> {
        let mut all_rules = Vec::new();

        // 1. Load Global Rules (~/.tempest/rules)
        if let Some(home) = dirs::home_dir() {
            let global_dir = home.join(".tempest").join("rules");
            if !global_dir.exists() {
                let _ = fs::create_dir_all(&global_dir);
            }
            if let Ok(rules) = self.load_from_dir(&global_dir) {
                all_rules.extend(rules);
            }
        }

        // 2. Load Project Rules (.tempest/rules)
        let local_dir = Path::new(".").join(".tempest").join("rules");
        if let Ok(rules) = self.load_from_dir(&local_dir) {
            for local_rule in rules {
                // Local overrides global with same name
                if let Some(pos) = all_rules.iter().position(|r| r.name == local_rule.name) {
                    all_rules[pos] = local_rule;
                } else {
                    all_rules.push(local_rule);
                }
            }
        }

        self.rules = all_rules;
        Ok(())
    }

    fn load_from_dir(&self, dir: &Path) -> Result<Vec<Rule>> {
        let mut rules = Vec::new();
        if !dir.exists() {
            return Ok(rules);
        }

        for entry in fs::read_dir(dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md")
                && let Ok(content) = fs::read_to_string(&path)
                && let Some(rule) = self.parse_rule(&content)
            {
                rules.push(rule);
            }
        }
        Ok(rules)
    }

    fn parse_rule(&self, content: &str) -> Option<Rule> {
        if !content.starts_with("---") {
            return None;
        }

        let parts: Vec<&str> = content.split("---").collect();
        if parts.len() < 3 {
            return None;
        }

        let yaml = parts[1];
        let body = parts[2..].join("---").trim().to_string();

        let mut name = String::new();
        let mut description = None;
        let mut globs = None;
        let mut always_apply = None;

        for line in yaml.lines() {
            let line = line.trim();
            if let Some(stripped) = line.strip_prefix("name:") {
                name = stripped
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
            } else if let Some(stripped) = line.strip_prefix("description:") {
                description = Some(
                    stripped
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string(),
                );
            } else if let Some(stripped) = line.strip_prefix("globs:") {
                let glob_str = stripped.trim();
                if glob_str.starts_with('[') && glob_str.ends_with(']') {
                    let items: Vec<String> = glob_str[1..glob_str.len() - 1]
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .collect();
                    globs = Some(items);
                }
            } else if let Some(stripped) = line.strip_prefix("always_apply:") {
                always_apply = Some(stripped.trim().parse::<bool>().unwrap_or(false));
            }
        }

        if name.is_empty() {
            return None;
        }

        Some(Rule {
            name,
            description,
            globs,
            always_apply,
            content: body,
        })
    }

    pub fn get_active_rules(&self, current_files: &[String]) -> Vec<Rule> {
        let mut active = Vec::new();

        for rule in &self.rules {
            if rule.always_apply.unwrap_or(false) {
                active.push(rule.clone());
                continue;
            }

            if let Some(globs) = &rule.globs {
                'rule_loop: for glob in globs {
                    let regex_pattern = glob
                        .replace(".", "\\.")
                        .replace("*", ".*")
                        .replace("?", ".");
                    let regex_str = format!("(?i)^{}$", regex_pattern);

                    if let Ok(re) = Regex::new(&regex_str) {
                        for file in current_files {
                            if re.is_match(file) {
                                active.push(rule.clone());
                                break 'rule_loop;
                            }
                        }
                    }
                }
            }
        }

        active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rule() {
        let content = r#"---
name: "Rust Guidelines"
description: 'Rules for Rust projects'
globs: ["*.rs", "src/**/*.rs"]
always_apply: false
---
Use standard formatting and Clippy.
"#;
        let engine = RuleEngine::new();
        let rule = engine.parse_rule(content).unwrap();
        assert_eq!(rule.name, "Rust Guidelines");
        assert_eq!(rule.description.as_deref(), Some("Rules for Rust projects"));
        assert_eq!(
            rule.globs,
            Some(vec!["*.rs".to_string(), "src/**/*.rs".to_string()])
        );
        assert_eq!(rule.always_apply, Some(false));
        assert_eq!(rule.content, "Use standard formatting and Clippy.");
    }

    #[test]
    fn test_get_active_rules() {
        let mut engine = RuleEngine { rules: Vec::new() };
        engine.rules.push(Rule {
            name: "Always Apply Rule".to_string(),
            description: None,
            globs: None,
            always_apply: Some(true),
            content: "Always".to_string(),
        });
        engine.rules.push(Rule {
            name: "Rust Rule".to_string(),
            description: None,
            globs: Some(vec!["*.rs".to_string()]),
            always_apply: Some(false),
            content: "Rust".to_string(),
        });
        engine.rules.push(Rule {
            name: "Python Rule".to_string(),
            description: None,
            globs: Some(vec!["*.py".to_string()]),
            always_apply: Some(false),
            content: "Python".to_string(),
        });

        // Test with empty active files
        let active = engine.get_active_rules(&[]);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "Always Apply Rule");

        // Test with rust active files
        let active = engine.get_active_rules(&["src/main.rs".to_string()]);
        assert_eq!(active.len(), 2);
        assert!(active.iter().any(|r| r.name == "Always Apply Rule"));
        assert!(active.iter().any(|r| r.name == "Rust Rule"));
    }
}
