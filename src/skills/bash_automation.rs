pub const NAME: &str = "bash_automation";
pub const DESCRIPTION: &str = "Write robust Bash scripts with proper error handling and logging";
pub const INSTRUCTIONS: &str = r#"
## Template
```bash
#!/usr/bin/env bash
set -euo pipefail  # Exit on error, undefined vars, pipe failures
IFS=$'\n\t'        # Safer word splitting

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log()  { echo -e "${GREEN}[INFO]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*" >&2; }
err()  { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }

# Check dependencies
command -v curl >/dev/null 2>&1 || err "curl is required but not installed"

main() {
    log "Starting..."
    # Your logic here
    log "Done!"
}

main "$@"
```

## Best Practices
- Always use set -euo pipefail at the top
- Quote ALL variables: "$var" not $var
- Use [[ ]] for conditionals, not [ ]
- Use $(command) for command substitution, not backticks
- Write functions for reusable logic
- Use trap cleanup EXIT for cleanup on exit
- For temp files: tmpfile=$(mktemp) and clean up with trap

## Common Patterns
- Check if root: [[ $EUID -ne 0 ]] && err "Must run as root"
- Check file exists: [[ -f "$file" ]] || err "File not found: $file"
- Loop over files: for f in /path/*.txt; do ... done
- Read config: source /etc/myapp/config.env
"#;
