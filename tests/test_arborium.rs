fn language_for_extension(ext: &str) -> Option<arborium::tree_sitter::Language> {
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
        "zsh" => "zsh",
        "sshconfig" | "ssh-config" | "ssh" => "ssh-config",
        "md" | "markdown" => "markdown",
        "cmake" => "cmake",
        "fish" => "fish",
        "asm" | "s" | "assembly" => "asm",
        other => other,
    };
    arborium::get_language(lang_name)
}

#[test]
fn test_arborium_lang_mapping() {
    assert!(language_for_extension("rs").is_some());
    assert!(language_for_extension("rust").is_some());
    assert!(language_for_extension("py").is_some());
    assert!(language_for_extension("js").is_some());
    assert!(language_for_extension("ts").is_some());
    assert!(language_for_extension("tsx").is_some());
    assert!(language_for_extension("go").is_some());
    assert!(language_for_extension("yaml").is_some());
    assert!(language_for_extension("cpp").is_some());
    assert!(language_for_extension("sh").is_some());
    assert!(language_for_extension("cs").is_some());
    assert!(language_for_extension("toml").is_some());
    assert!(language_for_extension("zig").is_some());
    assert!(language_for_extension("zsh").is_some());
    assert!(language_for_extension("ssh-config").is_some());
    assert!(language_for_extension("md").is_some());
    assert!(language_for_extension("markdown").is_some());
    assert!(language_for_extension("cmake").is_some());
    assert!(language_for_extension("fish").is_some());
    assert!(language_for_extension("asm").is_some());
    assert!(language_for_extension("s").is_some());
}
