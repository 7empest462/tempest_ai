---
name: python_script
description: Create a well-structured Python script with argument parsing and error handling
---
## Steps
1. Ask the user what the script should do
2. Write the script using `extract_and_write` with these conventions:
   - Add a shebang line: `#!/usr/bin/env python3`
   - Use `argparse` for CLI arguments
   - Wrap main logic in `def main():` with `if __name__ == "__main__":`
   - Use `try/except` blocks for error handling
   - Add docstrings to all functions
3. Make it executable with `chmod` tool (mode "755")
4. Verify with `run_command`: `python3 <script> --help`

## Best Practices
- Use f-strings for formatting, never % or .format()
- Use pathlib.Path instead of os.path
- Use `sys.exit(1)` for error exits
- Print errors to stderr: `print("Error:", msg, file=sys.stderr)`
