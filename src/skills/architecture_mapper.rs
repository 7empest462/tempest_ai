pub const NAME: &str = "architecture_mapper";
pub const DESCRIPTION: &str = "Map out the structure of a repository and generate architectural diagrams";
pub const INSTRUCTIONS: &str = r#"
## Steps
1. Initial Sweep: Run ls -R or use the tree tool to understand the directory layout.
2. Identify Entry Points: Look for main.rs, index.js, app.py, etc., to find the starting execution points.
3. Map Dependencies: 
   - Check Cargo.toml, package.json, or requirements.txt for external crates/libraries.
   - For internal modules, look at mod declarations in Rust or import statements in Python/JS.
4. Build the Graph:
   - Trace the flow from user input -> core logic -> external tools/databases.
   - Use Mermaid.js syntax to draw a high-level diagram of the modules.
5. Summarize Ownership: Explain which module is responsible for state, which for hardware, and which for the UI.

## Mermaid Template
```mermaid
graph TD
    A[Entry Point] --> B[Core Logic]
    B --> C[Data Layer]
    B --> D[Hardware Interface]
    C --> E[(SQLite/DB)]
```

## Key Principles
- Focus on the "Big Picture" first.
- Identify the "Source of Truth" for application state.
- Note any tight coupling between modules that could be problematic for refactoring.
"#;
