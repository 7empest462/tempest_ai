use super::{AgentTool, ToolContext};
use arborium::tree_sitter::{Language, Node, Parser, Query, QueryCursor};
use async_trait::async_trait;
use miette::{IntoDiagnostic, Result, miette};
use ollama_rs::generation::tools::{ToolFunctionInfo, ToolInfo, ToolType};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
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
        "go" | "golang" => "go",
        "yaml" | "yml" => "yaml",
        "java" => "java",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "sh" | "bash" => "bash",
        "zsh" => "zsh",
        "sshconfig" | "ssh-config" | "ssh" => "ssh-config",
        "md" | "markdown" => "markdown",
        "cmake" => "cmake",
        "fish" => "fish",
        "asm" | "s" | "assembly" => "asm",
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

#[derive(Deserialize, JsonSchema)]
pub struct AstOutlineArgs {
    /// Path to the source file to analyze.
    pub path: String,
}

pub struct AstOutlineTool;

#[async_trait]
impl AgentTool for AstOutlineTool {
    fn name(&self) -> &'static str {
        "ast_outline"
    }
    fn description(&self) -> &'static str {
        "Parse a source file using tree-sitter and return a structural outline (functions, structs, classes, impls) with line numbers. Supports Rust, Python, JavaScript, TypeScript. Use this to understand code structure before editing."
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<AstOutlineArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: AstOutlineArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();

        let path = std::path::Path::new(&path_owned);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let language = language_for_extension(ext).ok_or_else(|| {
            miette!(
                "Unsupported file type: '.{}'. Supported: any language enabled in arborium config.",
                ext
            )
        })?;

        let source = tokio::task::spawn_blocking({
            let p = path_owned.clone();
            move || std::fs::read_to_string(&p).map_err(|e| miette!("Failed to read file: {}", e))
        })
        .await
        .map_err(|e| miette!("Task join error: {}", e))??;

        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| miette!("Parser init error: {}", e))?;

        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| miette!("Failed to parse file: {}", path_owned))?;

        let mut symbols = Vec::new();
        collect_symbols(tree.root_node(), source.as_bytes(), 0, &mut symbols);

        if symbols.is_empty() {
            Ok(format!(
                "📄 {} — No structural symbols found (file may be empty or contain only expressions).",
                path_owned
            ))
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
            Ok(header)
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct AstEditArgs {
    /// Path to the source file.
    pub path: String,
    /// The name of the function/struct/class to find (e.g. "calculate_optimal_ctx").
    pub symbol_name: String,
    /// New content to replace the entire symbol body with.
    pub new_content: String,
}

pub struct AstEditTool;

#[async_trait]
impl AgentTool for AstEditTool {
    fn name(&self) -> &'static str {
        "ast_edit"
    }
    fn description(&self) -> &'static str {
        "Replace an entire function, struct, or class by name using tree-sitter AST lookup. This is safer than line-based patch_file because it finds the symbol by name, not by brittle line numbers."
    }
    fn is_modifying(&self) -> bool {
        true
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<AstEditArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: AstEditArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();
        let target_name = typed_args.symbol_name;
        let new_content = typed_args.new_content;

        if new_content.contains("...existing code...") || new_content.contains("// unchanged") {
            return Err(miette!(
                "Guardrail: Placeholder detected. You must provide the full symbol content."
            ));
        }

        let path = std::path::Path::new(&path_owned);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let language = language_for_extension(ext).ok_or_else(|| {
            miette!(
                "Unsupported file type: '.{}'. Supported: any language enabled in arborium config.",
                ext
            )
        })?;

        let source = std::fs::read_to_string(&path_owned)
            .map_err(|e| miette!("Failed to read file: {}", e))?;

        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| miette!("Parser init error: {}", e))?;

        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| miette!("Failed to parse file: {}", path_owned))?;

        // Find the target symbol by walking the AST
        fn find_symbol<'a>(
            node: Node<'a>,
            source: &'a [u8],
            target: &str,
        ) -> Option<(usize, usize)> {
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

        let (start_byte, end_byte) = find_symbol(tree.root_node(), source.as_bytes(), &target_name)
            .ok_or_else(|| miette!("Symbol '{}' not found in {}. Use `ast_outline` first to see available symbols.", target_name, path_owned))?;

        // Perform the replacement
        let mut result = String::with_capacity(source.len());
        result.push_str(&source[..start_byte]);
        result.push_str(&new_content);
        result.push_str(&source[end_byte..]);

        std::fs::write(&path_owned, &result).map_err(|e| miette!("Failed to write file: {}", e))?;

        let old_lines = source[start_byte..end_byte].lines().count();
        let new_lines = new_content.lines().count();

        Ok(format!(
            "✅ AST Edit: Replaced symbol '{}' in {} ({} lines → {} lines)",
            target_name, path_owned, old_lines, new_lines
        ))
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct AstQueryArgs {
    /// File extension representing the language to query (e.g. "rs", "py", "ts", "js").
    pub language: String,
    /// The root directory to search in (e.g., "." or "src").
    pub path: String,
    /// The Tree-Sitter S-expression query.
    pub query: String,
}

pub struct AstQueryTool;

#[async_trait]
impl AgentTool for AstQueryTool {
    fn name(&self) -> &'static str {
        "ast_query"
    }
    fn description(&self) -> &'static str {
        "Advanced Semantic Code Search using Tree-Sitter S-expressions. Allows structural searching across a directory.
EXAMPLES (for Rust `rs`):
1. Find all functions taking a specific type (e.g. String):
   (function_item parameters: (parameters (parameter type: (_) @type (#match? @type \"String\")))) @func
2. Find all structs:
   (struct_item name: (type_identifier) @name) @struct
3. Find all impl blocks for a specific trait:
   (impl_item trait: (type_identifier) @trait (#eq? @trait \"AgentTool\")) @impl
NOTE: You MUST use a valid tree-sitter S-expression query. Only matches the specific language extension."
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<AstQueryArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: AstQueryArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();
        let lang_ext = typed_args.language.trim_start_matches('.');

        let language = language_for_extension(lang_ext)
            .ok_or_else(|| miette!("Unsupported file extension: '{}'. Supported: any language enabled in arborium config.", lang_ext))?;

        let query = Query::new(&language, &typed_args.query)
            .map_err(|e| miette!("Invalid Tree-Sitter query: {:?}", e))?;

        let walker = ignore::WalkBuilder::new(&path_owned).build();
        let mut results = Vec::new();
        let mut files_scanned = 0;
        let mut match_count = 0;

        for result in walker {
            let entry = match result {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext != lang_ext {
                    continue;
                }
            } else {
                continue;
            }

            files_scanned += 1;

            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let mut parser = Parser::new();
            if parser.set_language(&language).is_err() {
                continue;
            }

            let tree = match parser.parse(&source, None) {
                Some(t) => t,
                None => continue,
            };

            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

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
                        snippet.replace("\n", "\n    "),
                        suffix
                    ));
                    match_count += 1;
                }
            }

            if !file_matches.is_empty() {
                results.push(format!(
                    "📄 {}\n{}",
                    path.display(),
                    file_matches.join("\n")
                ));
            }
        }

        if results.is_empty() {
            Ok(format!(
                "No matches found for query in {} files scanned.",
                files_scanned
            ))
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

            Ok(format!(
                "🔍 AST Search Results ({} matches in {} files, {} total files scanned):\n\n{}",
                match_count, total_files_matched, files_scanned, display_results
            ))
        }
    }
}
