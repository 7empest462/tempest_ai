pub const NAME: &str = "security_auditor";
pub const DESCRIPTION: &str = "Audit source code for common security vulnerabilities (SAST)";
pub const INSTRUCTIONS: &str = r#"
## Steps
1. Initial Search: Look for common security-sensitive keywords in the whole repository:
   - "API_KEY", "SECRET", "PASSWORD", "ENV".
   - "TODO", "FIXME", "XXX" (to find unfinished security logic).
2. Hardhearted Keys Check: Use grep or search_dir to find strings that look like hex, base64, or UUIDs in the source (not in logs/config).
3. Logic Audit: 
   - SQL Injection: Check for concatenated strings in rusqlite or PG queries.
   - Shell Injection: Check std::process::Command for unsanitized user input.
   - Memory Safety: In Rust, check for unsafe blocks and verify they are // SAFETY: documented.
   - Error Handling: Look for unwrap() or expect() in production paths that could lead to DoS (Denial of Service).
4. Environment Check: Verify .env files or sensitive configs are in .gitignore.
5. Suggest Hardening: Provide a list of "Quick Wins" to fix the most critical bugs.

## Key Notes
- Focus on "User-Controlled Input" — if user input can reach a command or a query, it's a high risk.
- Check for dependency vulnerabilities: cargo audit (if available) or npm audit.
- Secure defaults: Are passwords hashed? Is HTTPS used? Is CORS configured?
"#;
