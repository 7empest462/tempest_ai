---
name: unit_testing
description: Write and run unit/integration tests for Rust and Python projects
---
## Rust (Cargo)
1. Add tests in `src/lib.rs` or `src/main.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
```
2. Run: `cargo test`
3. Run specific test: `cargo test <test_name>`
4. Show output: `cargo test -- --nocapture`

## Python (pytest)
1. Install: `pip install pytest`
2. Create `test_*.py` files:
```python
def test_addition():
    assert 1 + 1 == 2

def test_failure():
    import pytest
    with pytest.raises(ValueError):
        raise ValueError("Error")
```
3. Run: `pytest` or `python3 -m pytest`
4. Verbose: `pytest -v`

## Key Notes
- Mocking: Use `mock` (Python) or `mockall` (Rust).
- Test Coverage: Use `pytest-cov` (Python) or `cargo-tarpaulin` (Rust).
- Integration tests: Put in `tests/` directory (Rust).
- CI/CD: Always ensure tests pass before pushing.
