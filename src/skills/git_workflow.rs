pub const NAME: &str = "git_workflow";
pub const DESCRIPTION: &str = "Standard Git workflow for branching, committing, and pushing changes";
pub const INSTRUCTIONS: &str = r#"
## Branch Strategy
1. Check current state: git status && git branch -a
2. Create a feature branch: git checkout -b feature/<name>
3. Make changes, then stage: git add -A
4. Commit with conventional commit messages:
   - feat: <description> — new feature
   - fix: <description> — bug fix
   - refactor: <description> — code restructuring
   - docs: <description> — documentation
   - chore: <description> — maintenance
5. Push: git push origin feature/<name>
6. If merging to main: git checkout main && git merge feature/<name> && git push origin main

## Common Operations
- Undo last commit (keep changes): git reset --soft HEAD~1
- Stash work: git stash && git stash pop
- View recent history: git log --oneline -10
- Check diff: git diff --stat
- Force push (DANGEROUS): git push --force-with-lease origin <branch>
- Clean untracked files: git clean -fd

## Key Notes
- NEVER force push to main without user confirmation
- Always check git status before committing to avoid committing junk
- Use --force-with-lease instead of --force to prevent overwriting others' work
"#;
