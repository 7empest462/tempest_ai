# 🧩 Tempest External Skills

Welcome to the **Tempest AI External Skills** repository. While the core "Native Skills" are baked directly into the Tempest binary for maximum performance, you can add your own custom abilities by dropping Markdown files into this directory (or your local `~/.tempest/skills/` folder).

---

## 🚀 How to Add a New Skill

To create a new skill, simply create a `.md` file with a YAML frontmatter block that defines its identity.

### Example: `production_deployment.md`
```markdown
---
name: production_deploy
description: "Standard operating procedure for zero-downtime production deployments"
---

## Phase 1: Pre-flight Checks
1. Ensure `cargo test` passes with 100% coverage
2. Check `thermal_guard` status — do not deploy if host is >80°C
3. Verify `compiler_guard` has clear logs for the last 30 minutes

## Phase 2: Deployment
1. Run `git push production main`
2. Monitor `system_telemetry` for memory leaks in the new process
3. Verify endpoint health: `curl -I https://api.tempest.ai/health`
```

---

## 🛠️ Performance Notes
- **Baked vs. Loaded**: Skills in this folder are loaded dynamically at runtime. If you find a skill is used daily across all your projects, consider porting it to a Native Rust module in `src/skills/` for faster execution.
- **Safety**: Tempest will automatically refuse system-modifying actions within a skill if `Planning Mode` is locked.
- **Override**: If an external skill has the same `name` as a Native Skill, the Native Skill will take precedence to prevent hijacking core safety protocols.

---

**Empower your agent with custom workflows.** 🌪️🦾✨
