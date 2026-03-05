# Config, Logging, and Init

## Problem

`lf init` scaffolds `.lf/config.yaml` with explicit defaults like `yolo: false`.
Repo config overrides global for all scalar keys. A user's global `yolo: true`
gets stomped by a repo scaffold they didn't write. No warning, no visibility.

Root cause: the bug isn't in precedence logic. Repo-wins is the conventional
rule (Git, VS Code, most tools). The bug is that init scaffolds personal
preferences into repo config, blocking the user's global values from ever
taking effect.

Second issue: no way to see what happened. No `-v` flag, no conflict logging.
Default log level is `info`, which produces noise on every run but misses the
diagnostic signal you need when something's wrong.

## Approach

1. **Conventional merge** — repo wins for scalars, additive keys combine.
   Same as today. No per-key classification, no `USER_PREFERENCE_KEYS`.

2. **Lean init** — agent-guided. Only scaffolds repo properties into
   `.lf/config.yaml`. Personal preferences (`yolo`, `ide`, `chrome`, etc.)
   are never written to repo config. If `~/.lf/config.yaml` doesn't exist,
   the agent offers to create it.

3. **Verbosity tiers** — `-v`/`-vv` flags and `LF_LOG` env var. Default
   level drops from `info` to `warn`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Per-key classification (user-preference vs repo) | Fixes yolo stomping directly | Over-engineers the merge to compensate for init putting things where they don't belong. Conventional repo-wins + lean init is simpler. |
| Three-scope model (user < repo < managed) | Full VS Code-style control | No enterprise customers yet. |
| Per-key override syntax in YAML | Users choose which keys to force | Cognitive overhead on every config edit. |

## Key decisions

### Repo wins, always

Standard precedence: repo config overrides user-global config for all scalar
keys. This is the Git/VS Code convention. No special cases.

The `yolo: false` stomping is fixed by not scaffolding `yolo` in repo config,
not by inverting precedence for certain keys.

### Remove `push` config key

Currently parsed but unused. Auto-push is handled by `lf ops commit -p`.
There's no meaningful `push: false` — if you don't want to push, just don't.
Delete from Config struct.

### `pr` stays, lives in user config

`pr` controls auto-PR creation after push. Repo can set it (team visibility),
user can override (quick experiments). Since repo wins and init won't scaffold
it in repo config, a user's global `pr: false` takes effect unless the repo
explicitly sets `pr: true`.

### Delete `include_loopflow_doc`

Parsed but unused. Remove from Config struct and init template.

### Default log level drops to `warn`

Current default is `info`, which logs ~10 lines per invocation. Noise. At
`warn`, users see nothing unless something needs attention.

The context header (model, directions, area, budget) stays — it's printed via
`eprintln!`, not through tracing.

### `-v` flag via reload layer

```rust
/// Increase verbosity (-v, -vv)
#[arg(short, long, action = clap::ArgAction::Count)]
pub verbose: u8,
```

Initialize tracing with `tracing_subscriber::reload` at `warn`. After clap
parses, update the filter based on `-v` count. No raw args scanning.

```rust
let filter = match (std::env::var("LF_LOG"), cli.verbose) {
    (Ok(lf_log), _) => EnvFilter::new(lf_log),       // LF_LOG wins
    (_, 0) => match std::env::var("RUST_LOG") {
        Ok(rust_log) => EnvFilter::new(rust_log),
        Err(_) => EnvFilter::new("lf=warn,loopflow=warn"),
    },
    (_, 1) => EnvFilter::new("lf=info,loopflow=info"),
    (_, _) => EnvFilter::new("lf=debug,loopflow=debug"),
};
```

### `LF_LOG` env var for diagnostics

```bash
LF_LOG=debug lf implement          # all debug output
LF_LOG=config,agent lf implement   # category filtering
```

Category names map to Rust module paths:

| Category | Module |
|----------|--------|
| `config` | `loopflow::engine::config` |
| `agent` | `loopflow::engine::agent` |
| `prompt` | `loopflow::engine::launch` |
| `discovery` | `loopflow::lf::discovery` |

`LF_LOG` takes precedence over everything. `RUST_LOG` still works as fallback.

### Config conflict logging

When both scopes set the same key, log the resolution at info (visible with `-v`):

```
config: yolo = true (repo override; user default: false)
config: agent = codex (repo override; user default: claude)
```

### Agent-guided init

`lf init` is an agent-guided flow, not a static template dump.

**Repo config** (`.lf/config.yaml`): The agent detects repo context (language,
existing config, CI), proposes defaults for repo keys, and asks what to change.
Only repo properties get written:

```yaml
agent: claude

supported_harnesses:
  - claude

exclude:
  - "*.lock"
  - node_modules
  - .venv
```

No `yolo`, `pr`, `chrome`, `ide`, `autoprune`, or commented-out templates.

**User config** (`~/.lf/config.yaml`): The agent checks whether it exists.
If it does, mention it and move on — don't touch it. If it doesn't, offer to
create one with a few common preferences (yolo, ide). Keep this brief — two
questions max, not an interrogation.

The init prompt should include just enough awareness of the key classification
to route things correctly, not a full inventory. Something like:

> Repo config is for team conventions (agent, exclude, harnesses).
> Personal preferences (yolo, ide, chrome) go in ~/.lf/config.yaml.
> Repo wins when both set the same key — so don't put personal prefs in repo config.

### Wire up unused config fields

`pr`, `land`, `context`, `exclude` are parsed but unused in the engine.

- `pr`: check after successful agent exit + push in `run.rs`
- `land`: pass to `lf ops land`
- `context`/`exclude`: feed into `GatherContextOpts`

### Log level audit

~100 warn statements demoted to debug (recoverable situations). ~30 kept as
warn (things the user should act on). ~5 promoted to error (correctness
failures). ~100 new log statements added at info/debug.

Rule of thumb: **info** = what `lf` decided. **debug** = how it decided.
**warn** = the user should act. **error** = something broke.

## Scope

- In scope: lean init, `-v` flag with reload layer, `LF_LOG`, default level
  change, conflict logging, `push` removal, `include_loopflow_doc` removal,
  log audit, wiring unused fields
- Out of scope: `lf config` status command, managed/policy tier, config file
  watching, migration tooling

## Done when

```bash
# User yolo works when repo doesn't set it
echo 'yolo: true' > ~/.lf/config.yaml
# .lf/config.yaml has no yolo key
# Config resolves yolo=true

# -v shows config resolution
lf implement -v 2>&1 | grep "config:"

# Default is quiet
lf implement 2>&1 | wc -l  # minimal output

# LF_LOG works
LF_LOG=config lf implement 2>&1 | grep "config"

# cargo test passes
cargo test --all

# cargo clippy clean
cargo clippy -- -D warnings
```
