use serde_json::{json, Value};
use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};

pub struct ReadFileTool;

#[async_trait]
impl AgentTool for ReadFileTool {
    fn name(&self) -> &'static str { "read_file" }
    fn description(&self) -> &'static str { "Reads the contents of a file. Use this to examine code or configuration." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file to read." }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();
        
        let metadata = fs::metadata(&path_owned)?;
        if metadata.len() > 1_000_000 {
            anyhow::bail!("File too large ({} bytes). Max 1MB.", metadata.len());
        }
        
        fs::read_to_string(&path_owned).map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path_owned, e))
    }
}

pub struct WriteFileTool;

#[async_trait]
impl AgentTool for WriteFileTool {
    fn name(&self) -> &'static str { "write_file" }
    fn description(&self) -> &'static str { "Writes content to a file, creating directories if needed. Overwrites existing content." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file." },
                "content": { "type": "string", "description": "Full content to write." }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let content = args.get("content").and_then(|c| c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'content'"))?;
        
        let path_owned = shellexpand::tilde(path_str).to_string();
        let path = PathBuf::from(&path_owned);

        if content.contains("...existing code...") || content.contains("// rest of file") {
            anyhow::bail!("Guardrail: Placeholder detected. You must provide the full file content.");
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        fs::write(&path, content)?;
        Ok(format!("Successfully wrote {} bytes to {}", content.len(), path.display()))
    }
}

pub struct ListDirTool;

#[async_trait]
impl AgentTool for ListDirTool {
    fn name(&self) -> &'static str { "list_dir" }
    fn description(&self) -> &'static str { "Lists directory contents." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory to list (defaults to '.')" }
            }
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        let path_owned = shellexpand::tilde(path_str).to_string();
        
        let mut out = Vec::new();
        for entry in fs::read_dir(&path_owned)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            let kind = if meta.is_dir() { "DIR " } else { "FILE" };
            out.push(format!("[{}] {}", kind, entry.file_name().to_string_lossy()));
        }
        Ok(out.join("\n"))
    }
}

pub struct SearchFilesTool;

#[async_trait]
impl AgentTool for SearchFilesTool {
    fn name(&self) -> &'static str { "search_files" }
    fn description(&self) -> &'static str { "Search for files by name/pattern in the current project." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Search pattern for filenames" },
                "path": { "type": "string", "description": "Root directory (default '.')" }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let pattern = args.get("pattern").and_then(|p| p.as_str()).unwrap();
        let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        let path_owned = shellexpand::tilde(path_str).to_string();
        
        use walkdir::WalkDir;
        
        let mut matches = Vec::new();
        for entry in WalkDir::new(&path_owned).into_iter().filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy();
            if name.contains(pattern) {
                matches.push(entry.path().display().to_string());
            }
            if matches.len() > 100 { break; }
        }
        
        if matches.is_empty() {
            Ok("No files found matching pattern.".to_string())
        } else {
            Ok(matches.join("\n"))
        }
    }
}

pub struct AppendFileTool;

#[async_trait]
impl AgentTool for AppendFileTool {
    fn name(&self) -> &'static str { "append_file" }
    fn description(&self) -> &'static str { "Append content to the end of an existing file. Creates the file if it doesn't exist." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file to append to" },
                "content": { "type": "string", "description": "Content to append" }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let path = shellexpand::tilde(path_str).to_string();
        let content = args.get("content").and_then(|c| c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'content'"))?;

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        file.write_all(content.as_bytes())?;
        Ok(format!("✅ Appended {} bytes to {}", content.len(), path))
    }
}

pub struct PatchFileTool;

#[async_trait]
impl AgentTool for PatchFileTool {
    fn name(&self) -> &'static str { "patch_file" }
    fn description(&self) -> &'static str { "Surgically replaces a specific range of lines in a file. Lines are 1-indexed." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string", "description": "The path to the file." },
                "start_line": { "type": "integer", "description": "1-indexed starting line number." },
                "end_line": { "type": "integer", "description": "1-indexed ending line number (inclusive)." },
                "content": { "type": "string", "description": "New content to insert." }
            },
            "required": ["file_path", "start_line", "end_line", "content"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let path_str = args.get("file_path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'file_path'"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();
        let start_line = args.get("start_line").and_then(|v| v.as_u64()).ok_or_else(|| anyhow::anyhow!("Missing 'start_line'"))? as usize;
        let end_line = args.get("end_line").and_then(|v| v.as_u64()).ok_or_else(|| anyhow::anyhow!("Missing 'end_line'"))? as usize;
        let content = args.get("content").and_then(|c| c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'content'"))?;

        if content.contains("...existing code...") || content.contains("// unchanged") {
             anyhow::bail!("Guardrail: Placeholder detected. Full content required.");
        }

        if start_line == 0 || end_line < start_line {
            anyhow::bail!("Invalid line range.");
        }

        let file_content = fs::read_to_string(&path_owned)?;
        let lines: Vec<&str> = file_content.lines().collect();

        if start_line > lines.len() + 1 {
            anyhow::bail!("start_line out of bounds.");
        }

        let mut new_lines = Vec::new();
        for i in 1..start_line {
            if i - 1 < lines.len() {
                new_lines.push(lines[i - 1].to_string());
            }
        }
        new_lines.push(content.to_string());
        for i in (end_line + 1)..=lines.len() {
            new_lines.push(lines[i - 1].to_string());
        }

        let final_content = new_lines.join("\n") + "\n";
        fs::write(&path_owned, final_content)?;
        Ok(format!("✅ Patched {} from line {} to {}", path_owned, start_line, end_line))
    }
}

pub struct FindReplaceTool;

#[async_trait]
impl AgentTool for FindReplaceTool {
    fn name(&self) -> &'static str { "find_replace" }
    fn description(&self) -> &'static str { "Regex or literal find-and-replace across files." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File or directory path" },
                "find": { "type": "string", "description": "The pattern to find" },
                "replace": { "type": "string", "description": "The replacement string" },
                "is_regex": { "type": "boolean", "description": "Regex mode. Default: false" },
                "file_pattern": { "type": "string", "description": "Optional glob filter" }
            },
            "required": ["path", "find", "replace"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();
        let find = args.get("find").and_then(|f| f.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'find'"))?;
        let replace = args.get("replace").and_then(|r| r.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'replace'"))?;
        let is_regex = args.get("is_regex").and_then(|r| r.as_bool()).unwrap_or(false);
        let file_pattern = args.get("file_pattern").and_then(|f| f.as_str());

        let path = std::path::Path::new(&path_owned);
        let mut files_to_process = Vec::new();

        if path.is_file() {
            files_to_process.push(path.to_path_buf());
        } else if path.is_dir() {
            fn collect_files(dir: &std::path::Path, pattern: Option<&str>, out: &mut Vec<PathBuf>) {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            if !p.file_name().map_or(false, |n| n.to_string_lossy().starts_with('.')) {
                                collect_files(&p, pattern, out);
                            }
                        } else if p.is_file() {
                            if let Some(pat) = pattern {
                                let glob = pat.trim_start_matches('*');
                                if p.to_string_lossy().ends_with(glob) {
                                    out.push(p);
                                }
                            } else {
                                out.push(p);
                            }
                        }
                    }
                }
            }
            collect_files(path, file_pattern, &mut files_to_process);
        } else {
             anyhow::bail!("Path not found.");
        }

        let mut total = 0;
        let mut modified = 0;
        let mut summary = String::new();

        for file in &files_to_process {
            if let Ok(content) = fs::read_to_string(file) {
                let new_content = if is_regex {
                    let re = regex::Regex::new(find).map_err(|e| anyhow::anyhow!("Invalid regex: {}", e))?;
                    let count = re.find_iter(&content).count();
                    if count > 0 {
                        total += count;
                        modified += 1;
                        summary.push_str(&format!("  {} ({} matches)\n", file.display(), count));
                        re.replace_all(&content, replace).to_string()
                    } else { continue; }
                } else {
                    let count = content.matches(find).count();
                    if count > 0 {
                        total += count;
                        modified += 1;
                        summary.push_str(&format!("  {} ({} matches)\n", file.display(), count));
                        content.replace(find, replace)
                    } else { continue; }
                };
                fs::write(file, new_content)?;
            }
        }

        if total == 0 {
            Ok(format!("No matches found for '{}'.", find))
        } else {
            Ok(format!("✅ {} replacements in {} files:\n{}", total, modified, summary))
        }
    }
}
pub struct DiffFilesTool;

#[async_trait]
impl AgentTool for DiffFilesTool {
    fn name(&self) -> &'static str { "diff_files" }
    fn description(&self) -> &'static str { "Generates a unified diff between two local files. Useful for comparing versions or verifying changes." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file1": { "type": "string", "description": "The first (original) file path." },
                "file2": { "type": "string", "description": "The second (modified) file path." }
            },
            "required": ["file1", "file2"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let f1_str = args.get("file1").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'file1'"))?;
        let f2_str = args.get("file2").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'file2'"))?;

        let f1_path = shellexpand::tilde(f1_str).to_string();
        let f2_path = shellexpand::tilde(f2_str).to_string();

        let c1 = fs::read_to_string(&f1_path)?;
        let c2 = fs::read_to_string(&f2_path)?;

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
             Ok("Files are identical.".to_string())
        } else {
             Ok(format!("Unified Diff between {} and {}:\n\n{}", f1_path, f2_path, diff_str))
        }
    }
}
