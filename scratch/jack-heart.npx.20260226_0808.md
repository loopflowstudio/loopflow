# Skill injection: `.claude/commands/` → `.agents/skills/`

Move loopflow's skill injection from flat `.claude/commands/*.md` files to the Agent Skills open standard format at `.agents/skills/<name>/SKILL.md`. This aligns with the cross-agent skill standard, gains auto-discovery and richer frontmatter, and makes loopflow steps first-class citizens in the npx skills ecosystem.

## What to build

Change injection to write `.agents/skills/<name>/SKILL.md` instead of `.claude/commands/<name>.md`. Project loopflow's frontmatter to SKILL.md format on injection — extract description, map `interactive` to `disable-model-invocation`, map directions to `user-invocable: false`. Keep loopflow's step resolution chain unchanged.

## Context

### Agent support tiers

| Agent | Tier | Skill path |
|-------|------|------------|
| **Claude Code** | Full | `.claude/skills/`, `.agents/skills/` |
| **Codex CLI** | Full | `.agents/skills/`, `.claude/skills/` |
| **OpenCode** | Full | `.opencode/skills/`, `.agents/skills/`, `.claude/skills/` |
| **Gemini CLI** | Experimental | `.gemini/commands/*.toml` (different format) |

All three fully-supported agents read from `.agents/skills/` — the [Agent Skills](https://agentskills.io) open standard path. One write covers Claude Code, Codex, and OpenCode simultaneously.

Gemini CLI uses TOML for custom commands and has partial Agent Skills support. It's experimental — we won't maintain a separate TOML projection.

### Why `.agents/skills/` over `.claude/skills/`

- Universal: all three primary agents read it
- Standard: the Agent Skills open standard path
- Neutral: not tied to any single agent's namespace
- Compatible: `npx skills add` installs here too

The [npx skills](https://github.com/vercel-labs/skills) ecosystem installs to `.agents/skills/` (and optionally per-agent paths). By injecting there, loopflow steps become visible to `npx skills list` and compatible with the broader ecosystem.

## Architecture

### Resolution (updated)

`lf` resolves steps in this order:

1. `.lf/steps/<name>.md` — repo-local loopflow steps
2. `.claude/commands/<name>.md` — repo-local legacy commands
3. `~/.lf/steps/<name>.md` — global loopflow steps
4. `~/.claude/commands/<name>.md` — global legacy commands
5. Built-in — compiled into binary
6. `.agents/skills/<name>/SKILL.md` — user-installed agent skills

Directions follow the same fallback: `.lf/directions/` → builtins → `.agents/skills/<name>/SKILL.md`.

This means `npx skills add` installs once, then `lf <name>` works without a prefix.

### Injection (changed)

Currently: `inject_skills()` writes flat `.md` files to `.claude/commands/`.

New: `inject_skills()` writes `SKILL.md` files to `.agents/skills/<name>/SKILL.md`.

```
Before:
  .claude/commands/design.md
  .claude/commands/implement.md
  .claude/commands/direction-care.md

After:
  .agents/skills/design/SKILL.md
  .agents/skills/implement/SKILL.md
  .agents/skills/direction-care/SKILL.md
```

### Frontmatter projection

Loopflow's step frontmatter is a superset. On injection, project down to SKILL.md format:

**Loopflow source** (`.lf/steps/review.md`):
```yaml
---
model: claude:sonnet
directions: [ux, craft]
interactive: true
requires: diff vs main
produces: verdict
---
Walk the human through the current diff and help them decide the next right move.
```

**Projected SKILL.md** (`.agents/skills/review/SKILL.md`):
```yaml
---
name: review
description: Walk the human through the current diff and help them decide the next right move.
model: claude:sonnet
---
Walk the human through the current diff and help them decide the next right move.
```

**Mapping rules:**

| Loopflow field | SKILL.md field | Rule |
|---|---|---|
| filename | `name` | Derive from step name |
| first line of body | `description` | Extract opening sentence |
| `interactive: true` | (omit `disable-model-invocation`) | Default — Claude can auto-invoke |
| `interactive: false` or absent for action steps | `disable-model-invocation: true` | User must `/invoke` |
| directions (`care`, `clarity`, etc.) | `user-invocable: false` | Background knowledge, not user actions |
| `model` | `model` | Pass through (may need format translation) |
| `directions`, `action_style`, `requires`, `produces` | (dropped) | Loopflow-only fields |
| Any SKILL.md-native field in source | (pass through) | `allowed-tools`, `context`, `agent`, etc. |

### What this enables

- **Cross-agent**: One write to `.agents/skills/` works for Claude Code, Codex, and OpenCode simultaneously.
- **Auto-discovery**: Agents load step descriptions into context and auto-invoke when relevant. A user asking "review my code" triggers `/review` without typing it.
- **npx ecosystem**: `npx skills list` shows loopflow steps. Users familiar with the skills ecosystem see loopflow steps alongside their other skills.
- **Supporting files**: The directory format allows steps to include templates, scripts, examples in subdirectories.
- **No-overwrite rule preserved**: `write_if_absent` still skips existing files. If a user installs a skill named `review` via npx, loopflow won't clobber it.

## Data structures

```rust
// Existing — no changes needed
pub struct Step {
    pub name: String,
    pub model: Option<String>,
    pub directions: Vec<String>,
    pub action_style: Option<String>,
    pub interactive: Option<bool>,
    pub content: Option<String>,
}

// New — frontmatter fields we parse but only use during projection
struct StepFrontmatter {
    // Existing loopflow fields
    model: Option<String>,
    directions: Vec<String>,
    action_style: Option<String>,
    interactive: Option<bool>,
    // SKILL.md fields (pass-through on projection)
    description: Option<String>,
    disable_model_invocation: Option<bool>,
    user_invocable: Option<bool>,
    allowed_tools: Option<String>,
    context: Option<String>,
    agent: Option<String>,
    argument_hint: Option<String>,
}
```

## Key functions

```rust
/// Project a loopflow step to SKILL.md format.
fn project_to_skill_md(name: &str, content: &str) -> String

/// Extract description from the first non-empty line after frontmatter.
fn extract_description(body: &str) -> Option<String>

/// Write skill to .agents/skills/<name>/SKILL.md if directory doesn't exist.
fn write_skill_if_absent(skills_dir: &Path, name: &str, content: &str) -> Option<PathBuf>
```

## Constraints

- **No-clobber**: Never overwrite user-installed skills. Check for directory existence, not just file.
- **Cleanup**: `cleanup_injected_skills` must remove directories, not just files. Track injected dirs for cleanup.
- **Namespaced steps**: `scan/scan-report` flattens to `scan-scan-report` directory name (same rule as today, applied to directory name instead of filename).
- **Directions prefix**: Directions inject as `direction-<name>` (e.g., `direction-care/SKILL.md`).

## Phase 2: `npx:` skill source

Register `npx` as a skill source prefix. `lf npx:<name>` fetches and runs skills from the npx ecosystem, with `.agents/skills/` as the cache.

### Usage

```bash
lf npx:explain-code                        # short name
lf npx:vercel-labs/agent-skills            # full repo path
```

### Resolution chain (fast-first)

1. **Cache hit** — `.agents/skills/<name>/SKILL.md` exists → load instantly
2. **Exact install** — `npx skills add <name>` → fast if it's a known package
3. **Search fallback** — `npx skills find <name>` → slower, may prompt for disambiguation

Try each in order. Stop at the first success.

### Caching

`.agents/skills/` IS the cache. No separate cache directory needed.

- First `lf npx:explain-code`: fetches via npx, lands in `.agents/skills/explain-code/SKILL.md`
- Second `lf npx:explain-code`: cache hit, instant
- `npx skills add some-skill` run manually: pre-warms the cache — `lf npx:some-skill` works on first try
- All agents see cached skills immediately

### Loopflow injection marker

Injected steps (phase 1) get a `loopflow: true` frontmatter field. The `npx:` source skips skills with this marker to avoid circular discovery — loopflow steps injected as skills shouldn't be re-discovered as npx skills.

### Implementation

Register `npx` as a `SkillSource` in `discover_skill_sources()`. Unlike directory-based sources (superpowers), the npx source has a fetch-on-miss fallback:

```rust
fn find_npx_skill(name: &str, repo: &Path) -> Option<Step> {
    let skills_dir = repo.join(".agents/skills");
    let skill_path = skills_dir.join(name).join("SKILL.md");

    // 1. Cache hit
    if skill_path.exists() {
        if has_loopflow_marker(&skill_path) {
            return None; // skip our own injections
        }
        return load_skill_from_path(&skill_path);
    }

    // 2. Exact install
    if run_npx_add(name).is_ok() {
        if skill_path.exists() {
            return load_skill_from_path(&skill_path);
        }
    }

    // 3. Search fallback
    if let Some(repo_path) = run_npx_find(name) {
        if run_npx_add(&repo_path).is_ok() {
            if skill_path.exists() {
                return load_skill_from_path(&skill_path);
            }
        }
    }

    None
}
```

### README update

Make `npx:` the primary skill source in documentation. Replace `sp:` examples with `npx:` as the default way to use external skills.

```bash
lf npx:explain-code          # grab a skill and run it
lf npx:deep-research -c      # with clipboard context
lf design                    # built-in step
```

`sp:` (superpowers) and other skill sources remain supported but aren't the lead example.

### Done when (phase 2)

- `lf npx:explain-code` loads from `.agents/skills/` cache
- Cache miss triggers `npx skills add` subprocess
- Search fallback runs `npx skills find` if exact add fails
- Loopflow-injected skills (with `loopflow: true` marker) are skipped
- Works across Claude Code, Codex, and OpenCode
- README updated with `npx:` as primary skill source examples

---

## Done when (phase 1)

- `inject_skills()` writes to `.agents/skills/<name>/SKILL.md` instead of `.claude/commands/<name>.md`
- Injected SKILL.md files have valid frontmatter with `name`, `description`, and `loopflow: true`
- `interactive: true` steps don't have `disable-model-invocation`
- Direction steps have `user-invocable: false`
- `cleanup_injected_skills()` removes injected directories
- Existing tests updated; new test for frontmatter projection
- `cargo test` and `cargo clippy` pass
