// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See LICENSE in the project root for full license information.

use miette::{Result, miette};
use serde::{Deserialize, Serialize};
use std::fs;
use tokio::process::Command;

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
                let content = tokio::task::spawn_blocking(move || {
                    fs::read_to_string(&path_expanded)
                        .map_err(|e| miette!("Failed to read file {}: {}", path, e))
                })
                .await
                .map_err(|e| miette!("Spawn blocking error: {}", e))??;
                Ok(content)
            }
            TempestEffect::WriteFile {
                path,
                content,
                force_overwrite,
            } => {
                let path_expanded = shellexpand::tilde(&path).to_string();
                let path_buf = std::path::PathBuf::from(&path_expanded);

                // Destructive write check (if file exists)
                if path_buf.exists() && !force_overwrite {
                    let old_metadata = fs::metadata(&path_buf)
                        .map_err(|e| miette!("Failed to read metadata for {}: {}", path, e))?;
                    let old_len = old_metadata.len();
                    let new_len = content.len() as u64;

                    let old_content_res = fs::read_to_string(&path_buf);
                    let old_line_count = old_content_res
                        .as_ref()
                        .map(|s| s.lines().count())
                        .unwrap_or(0);
                    let new_line_count = content.lines().count();

                    let is_destructive = (old_len > 100 && new_len < old_len / 2)
                        || (old_line_count > 5 && new_line_count == 1);

                    if is_destructive {
                        return Err(miette!(
                            "🛑 [DESTRUCTIVE WRITE BLOCKED]: This write would TRUNCATE '{}' from {} to {} bytes ({} to {} lines). \
                            This looks like an accidental overwrite. If this is intentional, re-call write_file with force_overwrite: true. \
                            For targeted edits, use edit_file_with_diff instead.",
                            path,
                            old_len,
                            new_len,
                            old_line_count,
                            new_line_count
                        ));
                    }
                }

                let p_clone = path.clone();
                tokio::task::spawn_blocking(move || {
                    if let Some(parent) = path_buf.parent() {
                        fs::create_dir_all(parent).map_err(|e| {
                            miette!("Failed to create parent directory for {}: {}", p_clone, e)
                        })?;
                    }
                    fs::write(&path_buf, content)
                        .map_err(|e| miette!("Failed to write file {}: {}", p_clone, e))?;
                    Ok::<(), miette::Report>(())
                })
                .await
                .map_err(|e| miette!("Spawn blocking error: {}", e))??;

                Ok(format!("Successfully wrote to {}", path))
            }
            TempestEffect::RunCommand { command, cwd } => {
                let mut cmd = if cfg!(target_os = "windows") {
                    let mut c = Command::new("cmd");
                    c.args(["/C", &command]);
                    c
                } else {
                    let mut c = Command::new("sh");
                    c.args(["-c", &command]);
                    c
                };
                cmd.current_dir(cwd);

                let output = cmd
                    .output()
                    .await
                    .map_err(|e| miette!("Failed to run command: {}", e))?;

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
