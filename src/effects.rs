use miette::{Result, miette};
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TempestEffect {
    ReadFile {
        path: String,
    },
    WriteFile {
        path: String,
        content: String,
        force_overwrite: bool,
    },
    RunCommand {
        command: String,
        cwd: String,
    },
}

pub struct TempestEffectExecutor;

impl Default for TempestEffectExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl TempestEffectExecutor {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute_effect(&self, effect: TempestEffect) -> Result<String> {
        match effect {
            TempestEffect::ReadFile { path } => {
                let path_expanded = shellexpand::tilde(&path).to_string();
                let content = fs::read_to_string(&path_expanded)
                    .map_err(|e| miette!("Failed to read file {}: {}", path, e))?;
                Ok(content)
            }
            TempestEffect::WriteFile {
                path,
                content,
                force_overwrite: _,
            } => {
                let path_expanded = shellexpand::tilde(&path).to_string();
                if let Some(parent) = std::path::Path::new(&path_expanded).parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        miette!("Failed to create parent directory for {}: {}", path, e)
                    })?;
                }
                fs::write(&path_expanded, content)
                    .map_err(|e| miette!("Failed to write file {}: {}", path, e))?;
                Ok(format!("Successfully wrote to {}", path))
            }
            TempestEffect::RunCommand { command, cwd } => {
                let output = if cfg!(target_os = "windows") {
                    Command::new("cmd")
                        .args(["/C", &command])
                        .current_dir(cwd)
                        .output()
                        .map_err(|e| miette!("Failed to run command: {}", e))?
                } else {
                    Command::new("sh")
                        .args(["-c", &command])
                        .current_dir(cwd)
                        .output()
                        .map_err(|e| miette!("Failed to run command: {}", e))?
                };

                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    Ok(stdout)
                } else {
                    Err(miette!(
                        "Command failed with exit code: {:?}\nError: {}",
                        output.status.code(),
                        stderr
                    ))
                }
            }
        }
    }
}
