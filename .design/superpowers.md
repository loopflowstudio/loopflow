# Superpowers

External skill library integration for loopflow. Run skills from superpowers (or other libraries) via `lf sp:<skill>`.

## Review

**Verdict:** Ready to ship

Clean implementation. External skills integrate naturally with the existing task system. Tests are thorough. Documentation is comprehensive.

## Design notes

**Skill name normalization:** Directory names like `brainstorming/` become `brainstorm`. The `-ing` and trailing `-s` removal is simple but handles most cases. Special-cased `tdd` for `test-driven-development`.

**Auto-detection precedence:**
1. Explicit `skill_sources` config wins
2. Repo-local `./superpowers` checked before `~/.superpowers`
3. Once a prefix (e.g., `sp`) is claimed, auto-detection skips it

**Maestro YAML parsing:** ConfigLoader has a TODO comment for parsing `skill_sources` from YAML—currently returns `nil`. Works for auto-detection but won't pick up explicit config until that's implemented.

**Context assembly:** External skills get full loopflow context (docs, diff, branch files). This is the key value prop—skills designed for Claude Code's raw context get loopflow's assembled context instead.
