# Config Scoping, Logging, and Init

## Problem

`lf init` scaffolds `.lf/config.yaml` with explicit defaults like `yolo: false`.
Repo config overrides global for all scalar keys. A user's global `yolo: true`
gets stomped by a repo scaffold they didn't write. No warning, no visibility.

Two deeper issues:

1. **All config keys follow the same precedence** (repo wins), but some keys are
   personal preferences and some are repo properties.

2. **No way to see what happened.** No `-v` flag, no config status command, no
   conflict logging. The default log level is `info`, which produces noise on
   every run but misses the diagnostic signal you need when something's wrong.

## Prior Art

### Config scoping

**VS Code** — 6 scopes (Default < User < Remote < Workspace < Folder < Policy).
Application-scoped settings (updates, telemetry, security) *cannot* live in
workspace config. Workspace Trust gates untrusted repos into Restricted Mode.

**Claude Code** — 4 scopes (User < Project < Local < Managed). Managed tier
can't be overridden. Permission arrays merge (concat + dedup). Security controls
like `disableBypassPermissionsMode` are managed-only. `claude /status` shows
resolved settings with origins.

**Cursor** — Inherits VS Code settings for editor. AI rules: Team > Project >
User > Legacy `.cursorrules`. Ships with Workspace Trust disabled by default
(led to CVE-2025-54135).

**Git** — system < global < local < worktree. Most specific always wins. No
concept of "this key can't be set locally."

### Logging and verbosity

**Cargo** — `-v`/`-vv` for user verbosity. `CARGO_LOG=trace` (not `RUST_LOG`)
for diagnostics. Separate env var prevents `.env` collisions.

**Git** — Per-category traces: `GIT_TRACE=1`, `GIT_TRACE_PERFORMANCE=1`,
`GIT_TRACE_SETUP=1`. Each writes to stderr or a file.

**Claude Code** — `--verbose` for turn-by-turn output. `--debug "api,mcp"` for
category filtering. Known problems: debug to stdout (not stderr), all-or-nothing
verbosity is the #1 user complaint.

**gh** — `GH_DEBUG=1` for general debug. `GH_DEBUG=api` for HTTP details.

### Patterns worth adopting

1. **Key-level scoping** — each setting has an owner (VS Code application scope)
2. **Conflict logging with origins** — show where values came from (Claude Code `/status`)
3. **Two verbosity layers** — user-facing (`-v`) vs diagnostic (env var)
4. **Namespaced env var** — `LF_LOG` not `RUST_LOG` (Cargo's `CARGO_LOG` pattern)
5. **Arrays merge, scalars replace** — (Claude Code permissions model)
6. **Tiered, not all-or-nothing** — (learning from Claude Code's mistake)

---

## Design: Config Scoping

### Scopes

Two scopes, same as today, with key-level precedence rules:

| Scope | File | Committed | Purpose |
|-------|------|-----------|---------|
| **User** | `~/.lf/config.yaml` | No | Personal preferences |
| **Repo** | `.lf/config.yaml` | Yes | Repo conventions |

No managed/policy tier for now. We're not an enterprise product yet.

### Key classification

Every config key belongs to one of three categories. The category determines
who wins when both scopes set the same key.

| Category | Repo can set? | User can set? | Conflict rule |
|----------|--------------|---------------|---------------|
| **user-preference** | Yes (as default) | Yes | User wins |
| **repo** | Yes | Yes (as default) | Repo wins |
| **additive** | Yes | Yes | Merge (concat + dedup) |

Both scopes can always set any key. Repo provides sensible defaults for
user-preference keys (useful for onboarding). User provides fallback defaults
for repo keys (useful for personal repos without `.lf/config.yaml`).

#### User-preference keys

| Key | Rationale |
|-----|-----------|
| `yolo` | Safety posture — user's choice about their own risk tolerance |
| `chrome` | Machine capability, not repo property |
| `ide` | Personal editor setup |
| `autoprune` | Personal workflow preference |
| `rlm_agent` | Personal model/cost choice for sub-agents |
| `rlm_max_parallel` | Machine resource constraint |
| `rlm_max_depth` | Personal recursion tolerance |

#### Repo keys

| Key | Rationale |
|-----|-----------|
| `agent` | Repo may require specific harness (e.g., codex for sandboxing) |
| `push` | Team may enforce push-on-commit |
| `pr` | Team may enforce auto-PR |
| `land` | Landing strategy depends on repo CI setup |
| `branch_names` | Repo naming convention |
| `release` | Release targets are repo structure |
| `area` | Default architectural scope |
| `interactive` | Which steps are interactive is a repo workflow choice |
| `budgets` | Token budgets tune for repo size |
| `summary_tokens` | Same |
| `lfdocs` | Whether lf docs matter for this repo |
| `diff` | Whether raw diff is useful for this repo's review process |
| `diff_files` | Same |
| `paste` | Some workflows depend on clipboard input |
| `include_loopflow_doc` | Whether loopflow docs are useful depends on repo |

#### Additive keys (unchanged)

| Key | Behavior |
|-----|----------|
| `context` | Global + repo lists combined |
| `exclude` | Global + repo lists combined |
| `summaries` | Global + repo lists combined |
| `skill_sources` | Global + repo lists combined |
| `supported_harnesses` | Global + repo lists combined |
| `direction` | User defaults + repo defaults combined |

### Merge implementation

```rust
const USER_PREFERENCE_KEYS: &[&str] = &[
    "yolo", "chrome", "ide", "autoprune",
    "rlm_agent", "rlm_max_parallel", "rlm_max_depth",
];

// In merge_config_values:
// 1. For USER_PREFERENCE_KEYS: user wins (global over repo)
// 2. For ADDITIVE_KEYS: combine lists (existing behavior)
// 3. For everything else: repo wins (existing behavior)
```

The Config struct doesn't change. The merge logic changes.

---

## Design: Logging

### Verbosity tiers

```bash
lf implement              # default: quiet (warnings/errors + context header)
lf implement -v           # verbose: config resolution, step discovery, timing
lf implement -vv          # very verbose: full prompt assembly, agent command
```

`-v` shows what `lf` is doing at a level a user would care about:
- Config loaded from X, merged with Y
- Config conflicts resolved (yolo = true from user, overrides repo false)
- Step discovered at path Z (repo override vs builtin)
- Agent launching with model M, yolo=true
- Prompt assembled: N tokens (area: X, docs: Y, diff: Z)
- Agent finished in Xs, exit code 0

`-vv` adds internals useful for bug reports:
- Full agent command line
- Prompt component sizes
- File-by-file context inclusion decisions
- Config file paths and existence

### Diagnostic logging: `LF_LOG`

```bash
LF_LOG=debug lf implement          # all debug output
LF_LOG=config,agent lf implement   # only config + agent modules
LF_LOG=trace lf implement          # everything including trace
```

`LF_LOG` takes precedence over `RUST_LOG`. `RUST_LOG` still works as fallback.

| Category | Module | What it shows |
|----------|--------|---------------|
| `config` | `loopflow::engine::config` | Config loading, merging, conflicts |
| `agent` | `loopflow::engine::agent` | Command building, launch args |
| `prompt` | `loopflow::engine::launch` | Prompt assembly, context gathering |
| `discovery` | `loopflow::lf::discovery` | Step/flow/direction resolution |
| `wave` | `loopflow::lfd::executor` | Wave execution, step orchestration |
| `trigger` | `loopflow::lfd::triggers` | Trigger evaluation, activation |
| `session` | `loopflow::lfd::sessions` | Session lifecycle, harness comms |
| `docker` | `loopflow::lfd::executor::docker` | Container operations |

### Default level: `warn`

Current default is `info`. Change to `warn`.

At `info`, every invocation logs ~10 lines of "preparing launch prompt",
"launching agent", "agent finished." Noise for someone who just wants to run
a step. At `warn`, they see nothing unless something is wrong.

The context header (model, directions, area, token budget) is already printed
via `eprintln!` and stays regardless of log level.

### CLI implementation

```rust
/// Increase verbosity (-v, -vv)
#[arg(short, long, action = clap::ArgAction::Count)]
pub verbose: u8,
```

Filter resolution:

```rust
let filter = match (std::env::var("LF_LOG"), cli.verbose) {
    (Ok(lf_log), _) => EnvFilter::new(lf_log),       // LF_LOG wins
    (_, 0) => match std::env::var("RUST_LOG") {        // RUST_LOG fallback
        Ok(rust_log) => EnvFilter::new(rust_log),
        Err(_) => EnvFilter::new("lf=warn,loopflow=warn"),
    },
    (_, 1) => EnvFilter::new("lf=info,loopflow=info"),
    (_, _) => EnvFilter::new("lf=debug,loopflow=debug"),
};
```

---

## Design: Conflict Logging

Conflicts are the first-class use case for logging. Anywhere two inputs
disagree, log the resolution.

### Config conflicts

When a key appears in both scopes, log at the appropriate level:

```rust
// User-preference key: user wins — log at info (visible with -v)
info!(key = "yolo", user = true, repo = false,
    "user preference overrides repo default");

// Repo key: repo wins — log at debug (visible with -vv)
debug!(key = "agent", repo = "codex", user = "claude:opus",
    "repo setting overrides user default");

// Additive key: merge — log at debug
debug!(key = "context", merged_count = 4,
    "combined user and repo lists");
```

At `-v`:
```
config: yolo = true (user override; repo default: false)
```

### CLI vs config conflicts

```rust
// CLI flag overrides config
if cli.yolo && !config.yolo {
    debug!("yolo: enabled via --yolo flag (config: false)");
}

// Model override
if cli.model.is_some() {
    debug!(cli_model = ?cli.model, config_model = ?config.agent,
        "model override via CLI flag");
}

// Step frontmatter vs config
if step.interactive != config.interactive.contains(&step.name) {
    debug!(step = step.name, "interactive conflict: using frontmatter");
}
```

### Config source tracking

`load_config` returns origin information alongside the config:

```rust
debug!(
    global_path = %global_path.display(),
    global_exists = global_data.is_some(),
    repo_path = ?repo_path,
    repo_exists = repo_data.is_some(),
    "config files"
);
```

This enables a future `lf config` command:

```
$ lf config
agent:       claude:opus    (repo: .lf/config.yaml)
yolo:        true           (user: ~/.lf/config.yaml, overrides repo: false)
chrome:      false          (default)
exclude:     [*.lock, node_modules, .venv]  (repo: .lf/config.yaml)
direction:   [clarity, care]  (merged: user + repo)
```

---

## Design: Init

`lf init` currently dumps everything into `.lf/config.yaml`. New behavior:

### Phase 1: Repo config (`.lf/config.yaml`)

Only repo-scoped keys. Lean.

```yaml
agent: claude

supported_harnesses:
  - claude

exclude:
  - "*.lock"
  - node_modules
  - .venv
```

No `yolo`, `push`, `pr`, `ide`, or commented-out templates.

### Phase 2: User config (`~/.lf/config.yaml`)

If `~/.lf/config.yaml` doesn't exist, offer to create it:

```
No user config found at ~/.lf/config.yaml

Set up personal preferences?
  Skip permission prompts (yolo)? [y/N]
  Auto-push after commits? [y/N]
```

Generate only what the user chose:

```yaml
yolo: true
```

If global config already exists, don't touch it.

### Phase 3: Optional extras (unchanged)

Superpowers, SkillRegistry, next steps — same as today.

---

## Log Level Audit

### Demote warn → debug (~100 statements)

Recoverable situations that don't need user attention:

- Token loading/refresh failures during startup recovery
- Container cleanup failures (best-effort operations)
- Missing optional files (wave config, scratch docs)
- Session recovery skips
- Docker image pull retries
- Trigger evaluation skips (no matching paths)

### Keep as warn (~30 statements)

Things the user should know about:

- Config conflicts on user-preference keys
- Agent exit with non-zero code
- Worktree in wrong location
- Failed to write prompt log
- Authentication token expiry

### Promote warn → error (~5 statements)

Failures that affect correctness:

- Wave status update failures
- Schema migration failures
- Stuck run detection and termination

### Add new logging (~100 statements)

| Area | Level | What |
|------|-------|------|
| Config merge | info | Conflict resolution for every key set in both scopes |
| Config files | debug | Which files exist, paths checked |
| Step discovery | info | Resolved step name, path, source (repo vs builtin) |
| Step discovery | debug | Search path, candidates checked |
| Agent launch | info | Harness, model, yolo, interactive |
| Agent launch | debug | Full command line, env vars |
| Prompt assembly | info | Token budget breakdown |
| Prompt assembly | debug | Per-component sizes, files included |
| Timing | debug | Elapsed ms at each milestone |
| Agent result | info | Exit code, elapsed time |

Rule of thumb: **info** = what `lf` decided. **debug** = how it decided.
**warn** = the user should act. **error** = something broke.

---

## Cleanup

The config audit found parsed-but-unused fields: `push`, `pr`, `land`,
`context`, `exclude`, `include_loopflow_doc`.

- **Keep and wire up**: `context`, `exclude` (additive, useful), `push`, `pr`,
  `land` (repo workflow settings).
- **Delete**: `include_loopflow_doc` (unclear purpose, unused).

---

## Summary

| What | Before | After |
|------|--------|-------|
| Config precedence | Repo wins for all scalars | User wins for preferences, repo wins for repo keys |
| Init scaffolds `yolo: false` | Yes | No — repo config is lean, user config is separate |
| Init creates user config | No | Yes, if `~/.lf/config.yaml` missing |
| Default log level | `info` | `warn` |
| User verbosity | `RUST_LOG=debug` | `lf -v` or `lf -vv` |
| Diagnostic logging | `RUST_LOG=...` | `LF_LOG=config,agent` (`RUST_LOG` still works) |
| Config conflict visibility | Silent | Logged at info (`-v`) with full resolution |
| Warn statements | ~135 | ~30 (rest demoted to debug) |
| Debug coverage | 55 statements | ~150+ |

## Rollout

1. Add `-v` flag and `LF_LOG` env var
2. Change default level to `warn`
3. Config merge: add `USER_PREFERENCE_KEYS`, flip precedence
4. Config conflict logging (the original bug fix)
5. Demote recoverable warns to debug
6. Add debug/info statements in gaps
7. Update init template (lean repo config, optional user config)
8. Update docs
