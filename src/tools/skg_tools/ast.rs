// ==========================================
// 🌲 SKG AST TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy AgentTool AST tools.

use arborium::tree_sitter::{Language, Node, Parser, Query, QueryCursor};
use skg_tool::ToolError;
use skg_tool_macro::skg_tool;
use streaming_iterator::StreamingIterator;

/// Detect the tree-sitter language from a file extension.
fn language_for_extension(ext: &str) -> Option<Language> {
    let lower = ext.to_lowercase();
    let lang_name = match lower.as_str() {
        "rs" | "rust" => "rust",
        "py" | "pyw" | "python" => "python",
        "js" | "mjs" | "cjs" | "javascript" => "javascript",
        "ts" | "typescript" => "typescript",
        "tsx" => "tsx",
        "jsx" => "jsx",
        "go" | "golang" => "go",
        "yaml" | "yml" => "yaml",
        "java" => "java",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "sh" | "bash" | "zsh" => "bash",
        "cs" | "csharp" => "c-sharp",
        "html" | "htm" => "html",
        "css" => "css",
        "c" => "c",
        "json" => "json",
        "hcl" | "tf" => "hcl",
        "lua" => "lua",
        "rb" | "ruby" => "ruby",
        "php" => "php",
        "toml" => "toml",
        "swift" => "swift",
        "kt" | "kts" | "kotlin" => "kotlin",
        "scala" | "sc" => "scala",
        "ps1" | "powershell" => "powershell",
        "ex" | "exs" | "elixir" => "elixir",
        "sql" => "sql",
        "starlark" | "bazel" | "bzl" => "starlark",
        "m" | "objc" => "objc",
        "xml" => "xml",
        "vue" => "vue",
        "dockerfile" | "docker" => "dockerfile",
        "zig" => "zig",
        other => other,
    };
    arborium::get_language(lang_name)
}

/// Recursively walk the AST and collect structural nodes (functions, structs, classes, etc.)
fn collect_symbols(node: Node, source: &[u8], depth: usize, symbols: &mut Vec<String>) {
    let kind = node.kind();

    // Collect meaningful structural nodes
    let is_structural = matches!(
        kind,
        "function_item"
            | "function_definition"
            | "function_declaration"
            | "struct_item"
            | "class_definition"
            | "class_declaration"
            | "impl_item"
            | "trait_item"
            | "enum_item"
            | "method_definition"
            | "arrow_function"
            | "mod_item"
            | "use_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "const_item"
            | "static_item"
            | "decorated_definition"
    );

    if is_structural {
        // Extract the name of the symbol
        let name = node
            .child_by_field_name("name")
            .map(|n| n.utf8_text(source).unwrap_or("?"))
            .unwrap_or_else(|| {
                // For impl blocks, try to get the type name
                node.child_by_field_name("type")
                    .map(|n| n.utf8_text(source).unwrap_or("?"))
                    .unwrap_or("(anonymous)")
            });

        let start = node.start_position();
        let end = node.end_position();
        let indent = "  ".repeat(depth);
        symbols.push(format!(
            "{}[L{}-L{}] {} `{}`",
            indent,
            start.row + 1,
            end.row + 1,
            kind,
            name
        ));
    }

    // Recurse into children
    let next_depth = if is_structural { depth + 1 } else { depth };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols(child, source, next_depth, symbols);
    }
}

// ── ast_outline ────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "ast_outline",
    description = "Parse a source file using tree-sitter and return a structural outline (functions, structs, classes, impls) with line numbers. Supports Rust, Python, JavaScript, TypeScript. Use this to understand code structure before editing."
)]
pub async fn ast_outline(path: String) -> Result<serde_json::Value, ToolError> {
    let path_owned = shellexpand::tilde(&path).to_string();
    let filepath = std::path::PathBuf::from(&path_owned);
    let ext = filepath.extension().and_then(|e| e.to_str()).unwrap_or("");

    let language = language_for_extension(ext).ok_or_else(|| {
        ToolError::ExecutionFailed(format!(
            "Unsupported file type: '.{}'. Supported: any language enabled in arborium config.",
            ext
        ))
    })?;

    let source = tokio::task::spawn_blocking({
        let p = path_owned.clone();
        move || std::fs::read_to_string(&p)
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))?
    .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file: {}", e)))?;

    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .map_err(|e| ToolError::ExecutionFailed(format!("Parser init error: {}", e)))?;

    let tree = parser.parse(&source, None).ok_or_else(|| {
        ToolError::ExecutionFailed(format!("Failed to parse file: {}", path_owned))
    })?;

    let mut symbols = Vec::new();
    collect_symbols(tree.root_node(), source.as_bytes(), 0, &mut symbols);

    if symbols.is_empty() {
        Ok(serde_json::Value::String(format!(
            "📄 {} — No structural symbols found (file may be empty or contain only expressions).",
            path_owned
        )))
    } else {
        let total_lines = source.lines().count();
        let header = format!(
            "📄 AST Outline: {} ({} lines, {} symbols)\n{}\n{}",
            path_owned,
            total_lines,
            symbols.len(),
            "─".repeat(50),
            symbols.join("\n")
        );
        Ok(serde_json::Value::String(header))
    }
}

// ── ast_edit ───────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "ast_edit",
    description = "Replace an entire function, struct, or class by name using tree-sitter AST lookup. This is safer than line-based patch_file because it finds the symbol by name, not by brittle line numbers."
)]
pub async fn ast_edit(
    path: String,
    symbol_name: String,
    new_content: String,
) -> Result<serde_json::Value, ToolError> {
    if new_content.contains("...existing code...") || new_content.contains("// unchanged") {
        return Err(ToolError::ExecutionFailed(
            "Guardrail: Placeholder detected. You must provide the full symbol content."
                .to_string(),
        ));
    }

    let path_owned = shellexpand::tilde(&path).to_string();
    let filepath = std::path::PathBuf::from(&path_owned);
    let ext = filepath.extension().and_then(|e| e.to_str()).unwrap_or("");

    let language = language_for_extension(ext).ok_or_else(|| {
        ToolError::ExecutionFailed(format!(
            "Unsupported file type: '.{}'. Supported: any language enabled in arborium config.",
            ext
        ))
    })?;

    let source = std::fs::read_to_string(&path_owned)
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file: {}", e)))?;

    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .map_err(|e| ToolError::ExecutionFailed(format!("Parser init error: {}", e)))?;

    let tree = parser.parse(&source, None).ok_or_else(|| {
        ToolError::ExecutionFailed(format!("Failed to parse file: {}", path_owned))
    })?;

    // Find the target symbol by walking the AST
    fn find_symbol<'a>(node: Node<'a>, source: &'a [u8], target: &str) -> Option<(usize, usize)> {
        let kind = node.kind();
        let is_structural = matches!(
            kind,
            "function_item"
                | "function_definition"
                | "function_declaration"
                | "struct_item"
                | "class_definition"
                | "class_declaration"
                | "impl_item"
                | "trait_item"
                | "enum_item"
                | "method_definition"
                | "arrow_function"
                | "mod_item"
                | "interface_declaration"
                | "type_alias_declaration"
                | "const_item"
                | "static_item"
        );

        if is_structural {
            let name = node
                .child_by_field_name("name")
                .or_else(|| node.child_by_field_name("type"))
                .map(|n| n.utf8_text(source).unwrap_or(""));

            if name == Some(target) {
                return Some((node.start_byte(), node.end_byte()));
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = find_symbol(child, source, target) {
                return Some(result);
            }
        }
        None
    }

    let (start_byte, end_byte) = find_symbol(tree.root_node(), source.as_bytes(), &symbol_name)
        .ok_or_else(|| {
            ToolError::ExecutionFailed(format!(
                "Symbol '{}' not found in {}. Use `ast_outline` first to see available symbols.",
                symbol_name, path_owned
            ))
        })?;

    // Perform the replacement
    let mut result = String::with_capacity(source.len());
    result.push_str(&source[..start_byte]);
    result.push_str(&new_content);
    result.push_str(&source[end_byte..]);

    std::fs::write(&path_owned, &result)
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {}", e)))?;

    let old_lines = source[start_byte..end_byte].lines().count();
    let new_lines = new_content.lines().count();

    Ok(serde_json::Value::String(format!(
        "✅ AST Edit: Replaced symbol '{}' in {} ({} lines → {} lines)",
        symbol_name, path_owned, old_lines, new_lines
    )))
}

// ── ast_query ──────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "ast_query",
    description = "Advanced Semantic Code Search using Tree-Sitter S-expressions. Allows structural searching across a directory. Supports Tree-Sitter S-expressions."
)]
pub async fn ast_query(
    language: String,
    path: String,
    query: String,
) -> Result<serde_json::Value, ToolError> {
    let path_owned = shellexpand::tilde(&path).to_string();
    let lang_ext = language.trim_start_matches('.');

    let ts_language = language_for_extension(lang_ext).ok_or_else(|| {
        ToolError::ExecutionFailed(format!(
            "Unsupported file extension: '{}'. Supported: any language enabled in arborium config.",
            lang_ext
        ))
    })?;

    let ts_query = Query::new(&ts_language, &query)
        .map_err(|e| ToolError::ExecutionFailed(format!("Invalid Tree-Sitter query: {:?}", e)))?;

    let walker = ignore::WalkBuilder::new(&path_owned).build();
    let mut results = Vec::new();
    let mut files_scanned = 0;
    let mut match_count = 0;

    for result in walker {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        let entry_path = entry.path();
        if !entry_path.is_file() {
            continue;
        }

        if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
            if ext != lang_ext {
                continue;
            }
        } else {
            continue;
        }

        files_scanned += 1;

        let source = match std::fs::read_to_string(entry_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut parser = Parser::new();
        if parser.set_language(&ts_language).is_err() {
            continue;
        }

        let tree = match parser.parse(&source, None) {
            Some(t) => t,
            None => continue,
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&ts_query, tree.root_node(), source.as_bytes());

        let mut file_matches = Vec::new();
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let start = capture.node.start_position();
                let end = capture.node.end_position();
                let text = &source[capture.node.start_byte()..capture.node.end_byte()];
                let snippet = text.lines().take(5).collect::<Vec<_>>().join("\n");
                let suffix = if text.lines().count() > 5 {
                    "\n..."
                } else {
                    ""
                };

                file_matches.push(format!(
                    "  [L{}-L{}] matched node:\n    {}{}",
                    start.row + 1,
                    end.row + 1,
                    snippet.replace('\n', "\n    "),
                    suffix
                ));
                match_count += 1;
            }
        }

        if !file_matches.is_empty() {
            results.push(format!(
                "📄 {}\n{}",
                entry_path.display(),
                file_matches.join("\n")
            ));
        }
    }

    if results.is_empty() {
        Ok(serde_json::Value::String(format!(
            "No matches found for query in {} files scanned.",
            files_scanned
        )))
    } else {
        let limit = 50;
        let total_files_matched = results.len();
        let display_results = if results.len() > limit {
            results.truncate(limit);
            format!(
                "{}\n\n... and {} more files.",
                results.join("\n\n"),
                total_files_matched - limit
            )
        } else {
            results.join("\n\n")
        };

        Ok(serde_json::Value::String(format!(
            "🔍 AST Search Results ({} matches in {} files, {} total files scanned):\n\n{}",
            match_count, total_files_matched, files_scanned, display_results
        )))
    }
}
