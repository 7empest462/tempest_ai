// ==========================================
// 📁 SKG FILE TOOLS — Native Skelegent Implementations
// ==========================================
// These replace the legacy AgentTool file tools with clean #[skg_tool] macros.
// Tool names match the legacy names exactly so the LLM's routing is preserved.

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use std::fs;
use std::path::PathBuf;

// ── read_file ──────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "read_file",
    description = "Reads the contents of a file. Use this to examine code or configuration."
)]
pub async fn read_file(
    path: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let path_owned = shellexpand::tilde(&path).to_string();

    let content = tokio::task::spawn_blocking(move || -> Result<String, ToolError> {
        let metadata = fs::metadata(&path_owned).map_err(|e| {
            ToolError::ExecutionFailed(format!("File not found or inaccessible: {}", e))
        })?;

        if metadata.len() > 1_000_000 {
            return Err(ToolError::ExecutionFailed(format!(
                "File too large: {} bytes (max 1MB)",
                metadata.len()
            )));
        }

        let bytes = fs::read(&path_owned).map_err(|e| {
            ToolError::ExecutionFailed(format!("IO error reading {}: {}", path_owned, e))
        })?;
        match String::from_utf8(bytes) {
            Ok(s) => Ok(s),
            Err(_) => Err(ToolError::ExecutionFailed(format!(
                "BINARY FILE DETECTED: '{}' cannot be read as text.",
                path_owned
            ))),
        }
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    Ok(serde_json::Value::String(content))
}

// ── write_file ─────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "write_file",
    description = "Writes content to a file, creating directories if needed. Overwrites existing content."
)]
pub async fn write_file(
    path: String,
    content: String,
    force_overwrite: Option<bool>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let force = force_overwrite.unwrap_or(false);

    if content.contains("...existing code...")
        || content.contains("// rest of file")
        || content.contains("// unchanged")
    {
        return Err(ToolError::ExecutionFailed(
            "Guardrail: Placeholder detected. You must provide the full file content. \
             Do NOT use ellipsis or comments as placeholders."
                .to_string(),
        ));
    }

    let path_owned = shellexpand::tilde(&path).to_string();
    let p = PathBuf::from(&path_owned);

    // 🛡️ Destructive write guard
    if let Ok(meta) = fs::metadata(&p) {
        let old_len = meta.len();
        let new_len = content.len() as u64;
        let old_line_count = fs::read_to_string(&p)
            .map(|s| s.lines().count())
            .unwrap_or(0);
        let new_line_count = content.lines().count();

        let is_destructive =
            (old_len > 100 && new_len < old_len / 2) || (old_line_count > 5 && new_line_count == 1);

        if is_destructive && !force {
            return Err(ToolError::ExecutionFailed(format!(
                "🛑 [DESTRUCTIVE WRITE BLOCKED]: This write would TRUNCATE '{}' from {} to {} bytes \
                 ({} to {} lines). If intentional, re-call with force_overwrite: true. \
                 For targeted edits, use edit_file_with_diff instead.",
                p.display(),
                old_len,
                new_len,
                old_line_count,
                new_line_count
            )));
        }
    }

    let content_clone = content.clone();
    let path_clone = p.clone();
    tokio::task::spawn_blocking(move || -> Result<(), ToolError> {
        if let Some(parent) = path_clone.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to create directory: {}", e))
            })?;
        }
        fs::write(&path_clone, &content_clone)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {}", e)))?;
        Ok(())
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    Ok(serde_json::json!({
        "status": "success",
        "message": format!("Successfully wrote {} bytes to {}", content.len(), path)
    }))
}

// ── list_dir ───────────────────────────────────────────────────────────────────

#[skg_tool(name = "list_dir", description = "Lists directory contents.")]
pub async fn list_dir(
    path: Option<String>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let mut path_val = path.unwrap_or_else(|| ".".to_string());
    if path_val.is_empty() {
        path_val = ".".to_string();
    }
    let path_owned = shellexpand::tilde(&path_val).to_string();

    let result = tokio::task::spawn_blocking(move || -> Result<String, ToolError> {
        let mut out = Vec::new();
        for entry in fs::read_dir(&path_owned)
            .map_err(|e| ToolError::ExecutionFailed(format!("Cannot read directory: {}", e)))?
        {
            let entry =
                entry.map_err(|e| ToolError::ExecutionFailed(format!("Dir entry error: {}", e)))?;
            let meta = entry
                .metadata()
                .map_err(|e| ToolError::ExecutionFailed(format!("Metadata error: {}", e)))?;
            let kind = if meta.is_dir() { "DIR " } else { "FILE" };
            out.push(format!(
                "[{}] {}",
                kind,
                entry.file_name().to_string_lossy()
            ));
        }
        Ok(out.join("\n"))
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    Ok(serde_json::Value::String(result))
}

// ── search_files ───────────────────────────────────────────────────────────────

#[skg_tool(
    name = "search_files",
    description = "Search for files by name/pattern in the current project."
)]
pub async fn search_files(
    pattern: String,
    path: Option<String>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let mut path_val = path.unwrap_or_else(|| ".".to_string());
    if path_val.is_empty() {
        path_val = ".".to_string();
    }
    let path_owned = shellexpand::tilde(&path_val).to_string();

    let result = tokio::task::spawn_blocking(move || -> Result<String, ToolError> {
        use fuzzy_matcher::FuzzyMatcher;
        use fuzzy_matcher::skim::SkimMatcherV2;
        use walkdir::WalkDir;

        let matcher = SkimMatcherV2::default();
        let mut matches = Vec::new();

        for entry in WalkDir::new(&path_owned)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "target" && name != "node_modules"
            })
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let name = entry.file_name().to_string_lossy();
                if let Some(score) = matcher.fuzzy_match(&name, &pattern) {
                    matches.push((score, entry.path().display().to_string()));
                }
            }
        }

        matches.sort_by_key(|entry| std::cmp::Reverse(entry.0));

        if matches.is_empty() {
            Ok("No files found matching pattern.".to_string())
        } else {
            let report = matches
                .into_iter()
                .take(50)
                .map(|(score, path)| format!("[{}] {}", score, path))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(report)
        }
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    Ok(serde_json::Value::String(result))
}

// ── diff_files ─────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "diff_files",
    description = "Generates a unified diff between two local files. Useful for comparing versions or verifying changes."
)]
pub async fn diff_files(
    file1: String,
    file2: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let f1_path = shellexpand::tilde(&file1).to_string();
    let f2_path = shellexpand::tilde(&file2).to_string();

    let c1 = fs::read_to_string(&f1_path)
        .map_err(|e| ToolError::ExecutionFailed(format!("Cannot read {}: {}", f1_path, e)))?;
    let c2 = fs::read_to_string(&f2_path)
        .map_err(|e| ToolError::ExecutionFailed(format!("Cannot read {}: {}", f2_path, e)))?;

    let mut diff_str = String::new();
    let diff = similar::TextDiff::from_lines(&c1, &c2);

    for (i, changeset) in diff.grouped_ops(3).iter().enumerate() {
        if i > 0 {
            diff_str.push_str("@@ ... @@\n");
        }
        for op in changeset {
            for change in diff.iter_changes(op) {
                let sign = match change.tag() {
                    similar::ChangeTag::Delete => "-",
                    similar::ChangeTag::Insert => "+",
                    similar::ChangeTag::Equal => " ",
                };
                diff_str.push_str(&format!("{}{}", sign, change.value()));
            }
        }
    }

    if diff_str.is_empty() {
        Ok(serde_json::Value::String(
            "Files are identical.".to_string(),
        ))
    } else {
        Ok(serde_json::Value::String(format!(
            "Unified Diff between {} and {}:\n\n{}",
            f1_path, f2_path, diff_str
        )))
    }
}

// ── append_file ────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "append_file",
    description = "Append content to the end of an existing file. Creates the file if it doesn't exist."
)]
pub async fn append_file(
    path: String,
    content: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let path_owned = shellexpand::tilde(&path).to_string();
    let content_owned = content;

    let result = tokio::task::spawn_blocking(move || -> Result<String, ToolError> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path_owned)
            .map_err(|e| ToolError::ExecutionFailed(format!("Cannot open file: {}", e)))?;
        file.write_all(content_owned.as_bytes())
            .map_err(|e| ToolError::ExecutionFailed(format!("Write failed: {}", e)))?;
        Ok(format!(
            "✅ Appended {} bytes to {}",
            content_owned.len(),
            path_owned
        ))
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    Ok(serde_json::Value::String(result))
}

// ── patch_file ─────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "patch_file",
    description = "Surgically replaces a specific range of lines in a file. Lines are 1-indexed."
)]
pub async fn patch_file(
    file_path: String,
    start_line: usize,
    end_line: usize,
    content: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    if content.contains("...existing code...") || content.contains("// unchanged") {
        return Err(ToolError::ExecutionFailed(
            "Guardrail: Placeholder detected. Full content required.".to_string(),
        ));
    }
    if start_line == 0 || end_line < start_line {
        return Err(ToolError::ExecutionFailed(
            "Invalid line range.".to_string(),
        ));
    }

    let path_owned = shellexpand::tilde(&file_path).to_string();

    let result = tokio::task::spawn_blocking(move || -> Result<String, ToolError> {
        let file_content = fs::read_to_string(&path_owned)
            .map_err(|e| ToolError::ExecutionFailed(format!("Cannot read file: {}", e)))?;
        let lines: Vec<&str> = file_content.lines().collect();

        if start_line > lines.len() + 1 {
            return Err(ToolError::ExecutionFailed(
                "start_line out of bounds.".to_string(),
            ));
        }

        let mut new_lines = Vec::new();
        for i in 1..start_line {
            if i - 1 < lines.len() {
                new_lines.push(lines[i - 1].to_string());
            }
        }
        new_lines.push(content);
        for i in (end_line + 1)..=lines.len() {
            new_lines.push(lines[i - 1].to_string());
        }

        let final_content = new_lines.join("\n") + "\n";
        fs::write(&path_owned, &final_content)
            .map_err(|e| ToolError::ExecutionFailed(format!("Write failed: {}", e)))?;

        Ok(format!(
            "✅ Patched {} from line {} to {}",
            path_owned, start_line, end_line
        ))
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    Ok(serde_json::Value::String(result))
}

// ── find_replace ───────────────────────────────────────────────────────────────

#[skg_tool(
    name = "find_replace",
    description = "Regex or literal find-and-replace across files."
)]
pub async fn find_replace(
    path: String,
    find: String,
    replace: String,
    is_regex: Option<bool>,
    file_pattern: Option<String>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let path_owned = shellexpand::tilde(&path).to_string();
    let is_regex = is_regex.unwrap_or(false);

    let result = tokio::task::spawn_blocking(move || -> Result<String, ToolError> {
        let p = std::path::Path::new(&path_owned);
        let mut files_to_process = Vec::new();

        if p.is_file() {
            files_to_process.push(p.to_path_buf());
        } else if p.is_dir() {
            fn collect_files(dir: &std::path::Path, pattern: Option<&str>, out: &mut Vec<PathBuf>) {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let ep = entry.path();
                        if ep.is_dir() {
                            if !ep
                                .file_name()
                                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                            {
                                collect_files(&ep, pattern, out);
                            }
                        } else if ep.is_file() {
                            if let Some(pat) = pattern {
                                let glob = pat.trim_start_matches('*');
                                if ep.to_string_lossy().ends_with(glob) {
                                    out.push(ep);
                                }
                            } else {
                                out.push(ep);
                            }
                        }
                    }
                }
            }
            collect_files(p, file_pattern.as_deref(), &mut files_to_process);
        } else {
            return Err(ToolError::ExecutionFailed("Path not found.".to_string()));
        }

        let mut total = 0;
        let mut modified = 0;
        let mut summary = String::new();

        for file in &files_to_process {
            if let Ok(content_str) = fs::read_to_string(file) {
                let new_content = if is_regex {
                    let re = regex::Regex::new(&find)
                        .map_err(|e| ToolError::ExecutionFailed(format!("Invalid regex: {}", e)))?;
                    let count = re.find_iter(&content_str).count();
                    if count > 0 {
                        total += count;
                        modified += 1;
                        summary.push_str(&format!("  {} ({} matches)\n", file.display(), count));
                        re.replace_all(&content_str, replace.as_str()).to_string()
                    } else {
                        continue;
                    }
                } else {
                    let count = content_str.matches(&find).count();
                    if count > 0 {
                        total += count;
                        modified += 1;
                        summary.push_str(&format!("  {} ({} matches)\n", file.display(), count));
                        content_str.replace(&find, &replace)
                    } else {
                        continue;
                    }
                };
                fs::write(file, &new_content)
                    .map_err(|e| ToolError::ExecutionFailed(format!("Write failed: {}", e)))?;
            }
        }

        if total == 0 {
            Ok(format!("No matches found for '{}'.", find))
        } else {
            Ok(format!(
                "✅ {} replacements in {} files:\n{}",
                total, modified, summary
            ))
        }
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    Ok(serde_json::Value::String(result))
}

// ── create_directory ───────────────────────────────────────────────────────────

#[skg_tool(
    name = "create_directory",
    description = "Creates a new directory and any necessary parent directories."
)]
pub async fn create_directory(
    path: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let path_owned = shellexpand::tilde(&path).to_string();

    tokio::task::spawn_blocking(move || -> Result<(), ToolError> {
        fs::create_dir_all(&path_owned)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create directory: {}", e)))
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    Ok(serde_json::json!({
        "status": "success",
        "message": format!("Successfully created directory: {}", path)
    }))
}

// ── delete_file ────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "delete_file",
    description = "Deletes a file or an empty directory. Use with caution."
)]
pub async fn delete_file(
    path: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let path_owned = shellexpand::tilde(&path).to_string();

    tokio::task::spawn_blocking(move || -> Result<(), ToolError> {
        let metadata = fs::metadata(&path_owned)
            .map_err(|e| ToolError::ExecutionFailed(format!("Not found: {}", e)))?;
        if metadata.is_dir() {
            fs::remove_dir(&path_owned)
                .map_err(|e| ToolError::ExecutionFailed(format!("Cannot remove dir: {}", e)))?;
        } else {
            fs::remove_file(&path_owned)
                .map_err(|e| ToolError::ExecutionFailed(format!("Cannot remove file: {}", e)))?;
        }
        Ok(())
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    Ok(serde_json::json!({
        "status": "success",
        "message": format!("Successfully deleted: {}", path)
    }))
}

// ── rename_file ────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "rename_file",
    description = "Renames or moves a file or directory to a new location."
)]
pub async fn rename_file(
    old_path: String,
    new_path: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let old = shellexpand::tilde(&old_path).to_string();
    let new = shellexpand::tilde(&new_path).to_string();

    tokio::task::spawn_blocking(move || -> Result<(), ToolError> {
        fs::rename(&old, &new)
            .map_err(|e| ToolError::ExecutionFailed(format!("Rename failed: {}", e)))
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    Ok(serde_json::json!({
        "status": "success",
        "message": format!("Successfully renamed {} to {}", old_path, new_path)
    }))
}

// ── extract_and_write ─────────────────────────────────────────────────────────

#[skg_tool(
    name = "extract_and_write",
    description = "Internal recovery tool. Extracts content from the assistant's previous turn and writes it to a file. Use this when write_file fails or is redirected."
)]
pub async fn extract_and_write(
    path: String,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx
        .deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    let path_owned = shellexpand::tilde(&path).to_string();
    let history = tool_ctx.history.lock();

    let re_markdown = regex::Regex::new(r"(?s)```(?:\w+)?\n(.*?)\n```")
        .map_err(|e| ToolError::ExecutionFailed(format!("Regex compile error: {}", e)))?;
    let re_json = regex::Regex::new(r#"(?s)"content"\s*:\s*"(.*?)""#)
        .map_err(|e| ToolError::ExecutionFailed(format!("Regex compile error: {}", e)))?;

    use ollama_rs::generation::chat::MessageRole;

    for msg in history.iter().rev() {
        if msg.role == MessageRole::Assistant {
            if let Some(cap) = re_markdown.captures(&msg.content) {
                let content = cap.get(1).unwrap().as_str();
                fs::create_dir_all(
                    PathBuf::from(&path_owned)
                        .parent()
                        .unwrap_or(&PathBuf::from(".")),
                )
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create dir: {}", e)))?;
                fs::write(&path_owned, content).map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to write file: {}", e))
                })?;
                return Ok(serde_json::Value::String(format!(
                    "Successfully extracted content from Markdown block and wrote to {}",
                    path_owned
                )));
            }

            if let Some(cap) = re_json.captures(&msg.content) {
                let content = cap
                    .get(1)
                    .unwrap()
                    .as_str()
                    .replace("\\n", "\n")
                    .replace("\\\"", "\"");
                fs::create_dir_all(
                    PathBuf::from(&path_owned)
                        .parent()
                        .unwrap_or(&PathBuf::from(".")),
                )
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create dir: {}", e)))?;
                fs::write(&path_owned, content).map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to write file: {}", e))
                })?;
                return Ok(serde_json::Value::String(format!(
                    "Successfully recovered content from raw JSON metadata and wrote to {}",
                    path_owned
                )));
            }
        }
    }

    Err(ToolError::ExecutionFailed(format!(
        "RECOVERY FAILED: Could not find suitable content in history to extract for {}.",
        path_owned
    )))
}
