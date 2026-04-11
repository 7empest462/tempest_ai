pub const NAME: &str = "debug_rust";
pub const DESCRIPTION: &str = "Systematic approach to debugging Rust compilation and runtime errors";
pub const INSTRUCTIONS: &str = r#"
## Compilation Errors
1. Read the FULL error message — Rust's compiler messages are extremely helpful
2. Common fixes:
   - E0277 (trait not implemented): Check if you need .get(), .into(), .as_ref(), or a use import
   - E0382 (moved value): Clone the value, use a reference &, or restructure ownership
   - E0599 (method not found): The trait providing the method isn't in scope — add use
   - E0308 (type mismatch): Check return types, use .to_string(), as u64, etc.
   - E0433 (unresolved import): Check Cargo.toml dependencies and feature flags
3. After fixing, always run cargo build to verify
4. If stuck, search the error code: rustc --explain E0277

## Runtime Errors / Panics
1. Run with backtrace: RUST_BACKTRACE=1 cargo run
2. Check for unwrap() calls on None/Err values — replace with proper error handling
3. Use cargo clippy for lint warnings
4. Run tests: cargo test

## Dependency Issues
1. Check versions: cargo tree -d (shows duplicates)
2. Update: cargo update
3. Check features: read the crate's docs.rs page
4. For proc-macro crates (procfs, sysinfo): check breaking API changes between versions

## Key Notes
- The procfs crate changed in recent versions: stat.rss_bytes returns an impl WithSystemInfo wrapper
  - Fix: call .get() on it, and add use procfs::WithCurrentSystemInfo;
- sysinfo API: component.temperature() now returns Option<f32> instead of f32
- Always check Cargo.lock diff when debugging sudden compilation failures
"#;
