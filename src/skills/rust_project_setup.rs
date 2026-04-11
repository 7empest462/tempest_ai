pub const NAME: &str = "rust_project_setup";
pub const DESCRIPTION: &str = "Create a new Rust project with best practices";
pub const INSTRUCTIONS: &str = r#"
## Steps
1. Use run_command with cargo init --name <project_name> <path> to scaffold the project
2. Read the generated Cargo.toml to verify the project structure
3. Add commonly needed dependencies based on the project type:
   - CLI app: clap, colored, anyhow
   - Web server: axum, tokio, serde
   - System tool: sysinfo, procfs (Linux), tokio
4. Create a proper .gitignore with /target and *.swp
5. Run cargo build to verify everything compiles
6. Initialize git with git init && git add -A && git commit -m "Initial commit"

## Key Learnings
- Always use anyhow::Result for error handling
- Prefer tokio runtime for async projects
- Use #[derive(Debug, Clone)] on all public structs
"#;
