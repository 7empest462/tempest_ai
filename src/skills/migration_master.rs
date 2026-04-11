pub const NAME: &str = "migration_master";
pub const DESCRIPTION: &str = "Migrate code across library versions or framework transitions";
pub const INSTRUCTIONS: &str = r#"
## Steps
1. Research Dependency Change:
   - Check Cargo.toml or package.json for the new version.
   - Use grep or search_web to find the official "Migration Guide" or breaking changes.
2. Isolate Affected Modules: 
   - Find all files using the library: grep -r "use <crate_name>" src/.
   - Identify which functions/structs are now invalid.
3. Draft the Transition:
   - Type Mapping: How do the new types correspond to the old ones (e.g., stat.rss_bytes -> stat.rss_bytes().get())?
   - API Refactor: Does the new API require async, different arguments, or Error types?
4. Iterative Refactor:
   - Modify ONE module at a time.
   - Run cargo check or tsc immediately to catch type errors.
   - Fix all compiler errors before moving to the next module.
5. Verify: Run tests and verify the logic with a tool or a sample run.

## Key Notes
- Don't touch what isn't broken: Only migrate where necessary unless a full upgrade is requested.
- Rollback Plan: Always keep a git stash or a separate branch in case the migration becomes too complex.
- Deprecation Warning: Mark old paths as #[deprecated] (Rust/JS) if you are doing a staged migration.
"#;
