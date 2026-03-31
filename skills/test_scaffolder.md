---
name: test_scaffolder
description: Automatically generate unit and integration test suites for functions and modules
---
## Steps
1. **Analyze Target**: Read the source file with `read_file` to understand the inputs, outputs, and side-effects of the function or struct.
2. **Identify Edge Cases**:
   - **Boundary Values**: What happens at 0, max_int, empty string, or empty Vec/List?
   - **Error Paths**: What happens when an `Ok/Some` is expected but an `Err/None` is received?
   - **Environment/State**: Does it depend on environment variables, file existence, or a database connection?
3. **Scaffold the Suite**:
   - Create a `#[cfg(test)] mod tests { ... }` block in Rust or a `tests/` file in Python/JS.
   - Use `mockall` or standard mocking libraries for trait/interface-heavy code.
4. **Iterative Test-Fix**:
   - Write one test at a time.
   - Run `cargo test` or `pytest`.
   - If a test fails, analyze if the bug is in the test or the target code.
5. **Verify Coverage**: Ensure the "happy path" and most critical "sad paths" (errors) are covered.

## Key Notes
- **Don't just test the Happy Path**.
- Use `assert_eq!`, `assert_ne!`, and `assert!(matches!(...))` for robust verification.
- For async code, use `#[tokio::test]`.
- Integration tests should be placed in `tests/` at the project root for Rust.
