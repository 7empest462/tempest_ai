use serde_json::Value;
use miette::{Result, IntoDiagnostic, miette};
use std::fs;
use std::path::PathBuf;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use ollama_rs::generation::chat::MessageRole;
use crate::error::FileError;

#[derive(Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// Path to the file to read.
    pub path: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct WriteFileArgs {
    /// Path to the file.
    pub path: String,
    /// Full content to write.
    pub content: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ListDirArgs {
    /// Directory to list (defaults to '.')
    pub path: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchFilesArgs {
    /// Search pattern for filenames
    pub pattern: String,
    /// Root directory (defaults to '.')
    pub path: Option<String>,
}

pub struct ReadFileTool;

#[async_trait]
impl AgentTool for ReadFileTool {
    fn name(&self) -> &'static str { "read_file" }
    fn description(&self) -> &'static str { "Reads the contents of a file. Use this to examine code or configuration." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<ReadFileArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: ReadFileArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();
        
        let metadata = fs::metadata(&path_owned).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => FileError::NotFound(path_owned.clone()),
            std::io::ErrorKind::PermissionDenied => FileError::PermissionDenied(path_owned.clone()),
            _ => FileError::Io { path: path_owned.clone(), source: e },
        })?;
        if metadata.len() > 1_000_000 {
            return Err(FileError::TooLarge { path: path_owned, size: metadata.len(), max: 1_000_000 }.into());
        }
        
        fs::read_to_string(&path_owned).map_err(|e| FileError::Io { path: path_owned, source: e }.into())
    }
}

pub struct WriteFileTool;

#[async_trait]
impl AgentTool for WriteFileTool {
    fn name(&self) -> &'static str { "write_file" }
    fn description(&self) -> &'static str { "Writes content to a file, creating directories if needed. Overwrites existing content." }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<WriteFileArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: WriteFileArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let content = typed_args.content;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();
        let path = PathBuf::from(&path_owned);

        if content.contains("...existing code...") || content.contains("// rest of file") {
            return Err(miette!("Guardrail: Placeholder detected. You must provide the full file content."));
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).into_diagnostic()?;
        }
        
        fs::write(&path, &content).into_diagnostic()?;
        Ok(format!("Successfully wrote {} bytes to {}", content.len(), path.display()))
    }
}

pub struct ListDirTool;

#[async_trait]
impl AgentTool for ListDirTool {
    fn name(&self) -> &'static str { "list_dir" }
    fn description(&self) -> &'static str { "Lists directory contents." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<ListDirArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: ListDirArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_val = typed_args.path.unwrap_or_else(|| ".".to_string());
        let path_owned = shellexpand::tilde(&path_val).to_string();
        
        let mut out = Vec::new();
        for entry in fs::read_dir(&path_owned).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let meta = entry.metadata().into_diagnostic()?;
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
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<SearchFilesArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: SearchFilesArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let pattern = typed_args.pattern;
        let path_val = typed_args.path.unwrap_or_else(|| ".".to_string());
        let path_owned = shellexpand::tilde(&path_val).to_string();
        
        use walkdir::WalkDir;
        
        let mut matches = Vec::new();
        for entry in WalkDir::new(&path_owned).into_iter().filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy();
            if name.contains(&pattern) {
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

#[derive(Deserialize, JsonSchema)]
pub struct AppendFileArgs {
    /// Path to the file to append to
    pub path: String,
    /// Content to append
    pub content: String,
}

pub struct AppendFileTool;

#[async_trait]
impl AgentTool for AppendFileTool {
    fn name(&self) -> &'static str { "append_file" }
    fn description(&self) -> &'static str { "Append content to the end of an existing file. Creates the file if it doesn't exist." }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<AppendFileArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: AppendFileArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path = shellexpand::tilde(&typed_args.path).to_string();
        let content = typed_args.content;

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path).into_diagnostic()?;
        file.write_all(content.as_bytes()).into_diagnostic()?;
        Ok(format!("✅ Appended {} bytes to {}", content.len(), path))
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct PatchFileArgs {
    /// The path to the file.
    pub file_path: String,
    /// 1-indexed starting line number.
    pub start_line: usize,
    /// 1-indexed ending line number (inclusive).
    pub end_line: usize,
    /// New content to insert.
    pub content: String,
}

pub struct PatchFileTool;

#[async_trait]
impl AgentTool for PatchFileTool {
    fn name(&self) -> &'static str { "patch_file" }
    fn description(&self) -> &'static str { "Surgically replaces a specific range of lines in a file. Lines are 1-indexed." }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<PatchFileArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: PatchFileArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_owned = shellexpand::tilde(&typed_args.file_path).to_string();
        let start_line = typed_args.start_line;
        let end_line = typed_args.end_line;
        let content = typed_args.content;

        if content.contains("...existing code...") || content.contains("// unchanged") {
             return Err(miette!("Guardrail: Placeholder detected. Full content required."));
        }

        if start_line == 0 || end_line < start_line {
            return Err(miette!("Invalid line range."));
        }

        let file_content = fs::read_to_string(&path_owned).into_diagnostic()?;
        let lines: Vec<&str> = file_content.lines().collect();

        if start_line > lines.len() + 1 {
            return Err(miette!("start_line out of bounds."));
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
        fs::write(&path_owned, final_content).into_diagnostic()?;
        Ok(format!("✅ Patched {} from line {} to {}", path_owned, start_line, end_line))
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct FindReplaceArgs {
    /// File or directory path
    pub path: String,
    /// The pattern to find
    pub find: String,
    /// The replacement string
    pub replace: String,
    /// Regex mode. Default: false
    pub is_regex: Option<bool>,
    /// Optional glob filter
    pub file_pattern: Option<String>,
}

pub struct FindReplaceTool;

#[async_trait]
impl AgentTool for FindReplaceTool {
    fn name(&self) -> &'static str { "find_replace" }
    fn description(&self) -> &'static str { "Regex or literal find-and-replace across files." }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<FindReplaceArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: FindReplaceArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();
        let find = &typed_args.find;
        let replace = &typed_args.replace;
        let is_regex = typed_args.is_regex.unwrap_or(false);
        let file_pattern = typed_args.file_pattern.as_deref();

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
             return Err(miette!("Path not found."));
        }

        let mut total = 0;
        let mut modified = 0;
        let mut summary = String::new();

        for file in &files_to_process {
            if let Ok(content) = fs::read_to_string(file) {
                let new_content = if is_regex {
                    let re = regex::Regex::new(find).map_err(|e| miette!("Invalid regex: {}", e))?;
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
                fs::write(file, new_content).into_diagnostic()?;
            }
        }

        if total == 0 {
            Ok(format!("No matches found for '{}'.", find))
        } else {
            Ok(format!("✅ {} replacements in {} files:\n{}", total, modified, summary))
        }
    }
}
#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct DiffFilesArgs {
    /// The first (original) file path.
    pub file1: String,
    /// The second (modified) file path.
    pub file2: String,
}

#[allow(dead_code)]
pub struct DiffFilesTool;

#[async_trait]
impl AgentTool for DiffFilesTool {
    fn name(&self) -> &'static str { "diff_files" }
    fn description(&self) -> &'static str { "Generates a unified diff between two local files. Useful for comparing versions or verifying changes." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<DiffFilesArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: DiffFilesArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let f1_path = shellexpand::tilde(&typed_args.file1).to_string();
        let f2_path = shellexpand::tilde(&typed_args.file2).to_string();

        let c1 = fs::read_to_string(&f1_path).into_diagnostic()?;
        let c2 = fs::read_to_string(&f2_path).into_diagnostic()?;

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

#[derive(Deserialize, JsonSchema)]
pub struct ExtractAndWriteArgs {
    /// Path to the file.
    pub path: String,
}

pub struct ExtractAndWriteTool;

#[async_trait]
impl AgentTool for ExtractAndWriteTool {
    fn name(&self) -> &'static str { "extract_and_write" }
    fn description(&self) -> &'static str { "Internal recovery tool. Extracts content from the assistant's previous turn and writes it to a file. Use this when write_file fails or is redirected." }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<ExtractAndWriteArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: ExtractAndWriteArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();
        let history = context.history.lock();
        
        for msg in history.iter().rev() {
            if msg.role == MessageRole::Assistant {
                let re_markdown = regex::Regex::new(r"(?s)```(?:\w+)?\n(.*?)\n```").into_diagnostic()?;
                if let Some(cap) = re_markdown.captures(&msg.content) {
                    let content = cap.get(1).unwrap().as_str();
                    fs::create_dir_all(PathBuf::from(&path_owned).parent().unwrap_or(&PathBuf::from(".")))
                        .into_diagnostic()?;
                    fs::write(&path_owned, content).into_diagnostic()?;
                    return Ok(format!("Successfully extracted content from Markdown block and wrote to {}", path_owned));
                }
                
                let re_json = regex::Regex::new(r#"(?s)"content"\s*:\s*"(.*?)""#).into_diagnostic()?;
                if let Some(cap) = re_json.captures(&msg.content) {
                    let content = cap.get(1).unwrap().as_str()
                        .replace("\\n", "\n")
                        .replace("\\\"", "\"");
                    fs::create_dir_all(PathBuf::from(&path_owned).parent().unwrap_or(&PathBuf::from(".")))
                        .into_diagnostic()?;
                    fs::write(&path_owned, content).into_diagnostic()?;
                    return Ok(format!("Successfully recovered content from raw JSON metadata and wrote to {}", path_owned));
                }
            }
        }
        
        Err(miette!("RECOVERY FAILED: Could not find suitable content in history to extract for {}.", path_owned))
    }
}
