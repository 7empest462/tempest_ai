use std::path::PathBuf;
use std::fs;
use serde::Deserialize;

pub mod debug_rust;
pub mod rust_project_setup;
pub mod unit_testing;
pub mod test_scaffolder;
pub mod bash_automation;
pub mod python_script;
pub mod api_server;
pub mod docker_deploy;
pub mod systemd_service;
pub mod launchd_service;
pub mod cron_scheduler;
pub mod dns_setup;
pub mod architecture_mapper;
pub mod migration_master;
pub mod network_scanner;
pub mod security_auditor;
pub mod server_hardening;
pub mod system_diagnostics;
pub mod web_scraper;
pub mod git_workflow;
pub mod task_complete;

#[derive(Debug, Deserialize, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub instructions: String,
}

#[derive(Clone, Debug)]
pub struct NativeSkill {
    pub name: &'static str,
    pub description: &'static str,
    pub instructions: &'static str,
}

pub fn get_native_skills() -> Vec<NativeSkill> {
    vec![
        NativeSkill { name: debug_rust::NAME, description: debug_rust::DESCRIPTION, instructions: debug_rust::INSTRUCTIONS },
        NativeSkill { name: rust_project_setup::NAME, description: rust_project_setup::DESCRIPTION, instructions: rust_project_setup::INSTRUCTIONS },
        NativeSkill { name: unit_testing::NAME, description: unit_testing::DESCRIPTION, instructions: unit_testing::INSTRUCTIONS },
        NativeSkill { name: test_scaffolder::NAME, description: test_scaffolder::DESCRIPTION, instructions: test_scaffolder::INSTRUCTIONS },
        NativeSkill { name: bash_automation::NAME, description: bash_automation::DESCRIPTION, instructions: bash_automation::INSTRUCTIONS },
        NativeSkill { name: python_script::NAME, description: python_script::DESCRIPTION, instructions: python_script::INSTRUCTIONS },
        NativeSkill { name: api_server::NAME, description: api_server::DESCRIPTION, instructions: api_server::INSTRUCTIONS },
        NativeSkill { name: docker_deploy::NAME, description: docker_deploy::DESCRIPTION, instructions: docker_deploy::INSTRUCTIONS },
        NativeSkill { name: systemd_service::NAME, description: systemd_service::DESCRIPTION, instructions: systemd_service::INSTRUCTIONS },
        NativeSkill { name: launchd_service::NAME, description: launchd_service::DESCRIPTION, instructions: launchd_service::INSTRUCTIONS },
        NativeSkill { name: cron_scheduler::NAME, description: cron_scheduler::DESCRIPTION, instructions: cron_scheduler::INSTRUCTIONS },
        NativeSkill { name: dns_setup::NAME, description: dns_setup::DESCRIPTION, instructions: dns_setup::INSTRUCTIONS },
        NativeSkill { name: architecture_mapper::NAME, description: architecture_mapper::DESCRIPTION, instructions: architecture_mapper::INSTRUCTIONS },
        NativeSkill { name: migration_master::NAME, description: migration_master::DESCRIPTION, instructions: migration_master::INSTRUCTIONS },
        NativeSkill { name: network_scanner::NAME, description: network_scanner::DESCRIPTION, instructions: network_scanner::INSTRUCTIONS },
        NativeSkill { name: security_auditor::NAME, description: security_auditor::DESCRIPTION, instructions: security_auditor::INSTRUCTIONS },
        NativeSkill { name: server_hardening::NAME, description: server_hardening::DESCRIPTION, instructions: server_hardening::INSTRUCTIONS },
        NativeSkill { name: system_diagnostics::NAME, description: system_diagnostics::DESCRIPTION, instructions: system_diagnostics::INSTRUCTIONS },
        NativeSkill { name: web_scraper::NAME, description: web_scraper::DESCRIPTION, instructions: web_scraper::INSTRUCTIONS },
        NativeSkill { name: git_workflow::NAME, description: git_workflow::DESCRIPTION, instructions: git_workflow::INSTRUCTIONS },
        NativeSkill { name: task_complete::NAME, description: task_complete::DESCRIPTION, instructions: task_complete::INSTRUCTIONS },
    ]
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
    let mut skills = Vec::new();

    // 1. Load Native Skills (Baked into binary)
    for ns in get_native_skills() {
        skills.push(Skill {
            name: ns.name.to_string(),
            description: ns.description.to_string(),
            instructions: ns.instructions.to_string(),
        });
    }

    // 2. Load External Skills (Dynamic from ~/.tempest/skills)
    let dir = skills_dir();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "md") {
                if let Ok(content) = fs::read_to_string(&path) {
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
                                // Only add if not already present in native skills (native overrides external with same name)
                                if !skills.iter().any(|s| s.name == name) {
                                    skills.push(Skill { name, description, instructions });
                                }
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
                        let mut topic = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                        for line in content.lines() {
                            if line.starts_with("topic:") {
                                topic = line["topic:".len()..].trim().to_string();
                                break;
                            }
                        }
                        
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
