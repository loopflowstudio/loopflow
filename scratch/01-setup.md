# Sprint 1: Cross-Platform Init

## Problem

`lf init` is the front door to loopflow and it's locked for half the audience. It hard-gates on macOS, requires Homebrew, tries to install things via `brew`/`npm`, doesn't know OpenCode exists, and hardcodes `claude:opus` as the default agent. A Linux user with OpenCode installed — a perfectly valid setup — bounces immediately.

Beyond init, the three setup entry points (`lf init`, `lfd install`, Concerto) have overlapping responsibilities and no documented handoff. A new user doesn't know which one to start with.

**Who benefits:** Every new user. Linux users are blocked entirely. macOS users without Homebrew are blocked. Users who prefer OpenCode or Codex get a Claude-centric experience that doesn't reflect what they have installed.

**Why now:** This is phase 01 of the docs wave. Everything downstream (workflow docs, wave authoring guide) assumes users can actually get started.

## Approach

Rewrite `init.md` as a detection-only prompt. Init never installs anything — it checks what's present, sets smart defaults, and tells the user what to do next. Three concrete deliverables:

### 1. Rewrite init.md

Replace the current 5-phase prompt with a new 5-phase prompt that works on any platform:

**Phase 1: Environment check**
- `git rev-parse --show-toplevel` — must be in a git repo
- `uname -s` — detect platform (for next-steps messaging, not gating)
- `command -v claude`, `command -v codex`, `command -v opencode` — detect installed agents
- `test -f .lf/config.yaml` — check existing config

No `command -v brew`, no `command -v npm`, no `command -v gemini`. Init doesn't care about package managers or unsupported agents.

Use `command -v` not `which` — it's a POSIX shell builtin, always available, consistent exit codes. `which` behavior varies across distros and may not be installed on minimal Linux systems.

**Phase 2: Agent guidance**
- Multiple found (2 or 3): report all, ask which to default
- One found: default to it silently, report the choice
- None found: show install instructions (not run them), stop

The "none found" message is platform-aware in tone but the install commands are the same everywhere:
```
No coding agent found. Install one:
  Claude Code:  npm install -g @anthropic-ai/claude-code
  OpenCode:     go install github.com/anomalyco/opencode@latest
  Codex CLI:    npm install -g @openai/codex
Then run `lf init` again.
```

**Phase 3: Create or update config**

If no `.lf/config.yaml` exists:
- Write a fresh config with detected agent as default
- `supported_harnesses` auto-populated from detection

If `.lf/config.yaml` already exists:
- Read it and compare `supported_harnesses` against detected agents
- If new agents are detected that aren't in `supported_harnesses`, offer to add them
- If the current `agent:` value isn't installed, offer to switch the default
- If everything matches, report "Config is up to date" and move on
- Never blow away custom `exclude` patterns or other user-tuned fields

No hardcoded `claude:opus` — use the detected agent name (e.g., `claude`, `opencode`, `codex`)

Config template:
```yaml
# Loopflow configuration
agent: <detected-default>

supported_harnesses:
  - <detected-agents>

context: "."

exclude:
  - "*.lock"
  - node_modules
  - .venv

yolo: false
push: false

skill_registry:
  enabled: false
```

**Phase 4: Optional extras**
- superpowers: offer `git clone` if `~/.superpowers` doesn't exist
- SkillRegistry: offer to enable in config
- Drop IDE preferences entirely (Warp/Cursor). Loopflow is editor-agnostic.

**Phase 5: Next steps**
Platform-aware guidance:
- macOS: "Download Concerto for visual wave management, or explore with `lf design`"
- Linux: "Set up tmux integration for wave monitoring, or explore with `lf design`"
- Both: "Run `lf <step>` to try individual steps, or `lfd install` to set up the daemon for autonomous waves"

### 2. Document setup entry points in getting-started.md

Add a "Setup" section at the top of getting-started.md with the routing table:

```
I want to...                    Start here
────────────────────────────────────────────
Try loopflow from terminal      lf init
Run autonomous waves            lf init → lfd install
Use the visual app (macOS)      Download Concerto (handles the rest)
Connect from iPhone             Concerto iOS → discovers your lfd
Set up remote dev server        lfd install --container on server
```

Update the existing "Install" section to drop the "Requires macOS" claim and mention Linux support. Remove references to Gemini CLI until it's supported.

### 3. Drop the flows README creation

The current init creates `.lf/flows/README.md` as a bonus. This is noise — flows are documented in the main docs. Init should create exactly one file: `.lf/config.yaml`. If a user needs flow docs, they'll find them.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep init as installer, add Linux package managers | Broader platform support but init stays fragile — every new distro needs new install logic | Init shouldn't install. Detection is reliable; installation is a moving target |
| Move init to compiled Rust code instead of a prompt | Deterministic, testable, no LLM variability | Overkill for this phase. The prompt approach works well for conversational setup. The risk is in what the prompt tells the agent to do, not the execution model |
| Auto-detect platform and install silently | Fastest path to working setup | Violates principle of least surprise. Users should consent to installations. Detection-only is safer and more trustworthy |

## Key decisions

**Detection, never installation.** The current init runs `brew install` and `npm install`. The new init only runs `command -v` and `test`. This is the single biggest change. It makes init safe to run on any platform because it can't break anything.

**No Gemini CLI.** The wave explicitly marks Gemini as "not here" — the harness doesn't exist yet. Current init checks for `which gemini` and offers to install it. Remove all references. Add back when the harness ships.

**Drop IDE preferences.** The Warp/Cursor questions in Phase 4 are vestigial. The `IdeConfig` struct exists in config.rs but init shouldn't be asking about editors. Loopflow runs in terminal; your editor is your business.

**Agent name without model as default.** Write `agent: claude` not `agent: claude:opus`. The engine already handles model defaults in `parse_agent()` — `claude` resolves to `opus`, `codex` resolves to its default, `opencode` to its config. Let users override the model if they want to; don't encode a specific model in the default config.

**One file output.** Init creates `.lf/config.yaml` and nothing else. No flows README, no IDE config, no extra scaffolding. The config file is the only artifact.

**Concerto gap is out of scope.** The SetupView/SetupService in Swift also hardcode macOS/Homebrew assumptions, but fixing Concerto setup is Concerto feature work — explicitly excluded by the wave vision ("Not here: Concerto feature development"). We note the gap for a future wave.

## Scope

**In scope:**
- Rewrite `rust/loopflow/src/engine/builtins/steps/ops/init.md`
- Update `docs/getting-started.md` with setup routing table and Linux support
- Update `README.md` requirements section: replace "macOS" with "macOS or Linux" and note that Concerto is macOS-only

**Out of scope:**
- Concerto SetupView/SetupService changes (wave says "not Concerto features")
- Gemini CLI support (wave says "not here")
- New steps or flows
- `lfd install` changes (already cross-platform: launchd on macOS, systemd on Linux)
- docs/ restructure (that's sprint 02)

## Done when

Advancing wave goals:
- *"`lf init` works on macOS and Linux with zero platform-specific dependencies"*
- *"Setup entry points (lf init, lfd install, Concerto) have clear ownership and hand off cleanly"*

Verification:

- [ ] `lf init` on macOS without Homebrew: succeeds, detects agents in PATH, writes config
- [ ] `lf init` on Linux (Ubuntu): succeeds, detects agents in PATH, writes config
- [ ] `lf init` with only OpenCode installed: defaults to `agent: opencode`, `supported_harnesses: [opencode]`
- [ ] `lf init` with nothing installed: shows install instructions, doesn't try to run package managers, stops
- [ ] `lf init` with multiple agents: asks which to default to
- [ ] `lf init` with existing `.lf/config.yaml`: detects new agents, offers to add to `supported_harnesses` without clobbering custom config
- [ ] `lf init` with existing config where `agent:` points to uninstalled agent: offers to switch default
- [ ] No references to Homebrew, `brew install`, `npm install`, `which`, or Gemini CLI in init.md
- [ ] getting-started.md has "I want to..." routing table
- [ ] README.md requirements section mentions Linux
- [ ] `cargo test -p loopflow golden_prompt` passes (if init has golden tests)

---

# `lf ops ingest` — fast-path for wave item ingestion

## Context

The `ingest` step currently spins up an LLM agent to pick the next wave item. In the common case, the logic is deterministic: find the lowest-numbered `.md` file in `wave/<wave>/`, copy it to `scratch/`, remove the original. PR #489 introduces fast-path infrastructure — steps can declare `fast-path: <shell command>` in frontmatter. Exit 0 skips the agent; non-zero falls through with failure context.

## Behavior

| Scenario | Exit | Effect |
|----------|------|--------|
| One file with lowest numeric prefix | 0 | Copy to `scratch/<wave>-<slug>.md`, remove original |
| Multiple files with same lowest prefix | 1 | Agent evaluates priority |
| No numbered items remaining | 1 | Signals wave complete |
| Wave dir not found / can't resolve name | 1 | Agent investigates |

## Files

### New: `rust/loopflow/src/ops/ingest.rs`

```rust
#[derive(Debug, Clone)]
pub struct IngestOptions {
    pub wave: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IngestResult {
    pub wave: String,
    pub slug: String,
    pub dest: PathBuf,
}

pub fn ingest(repo: &Path, options: &IngestOptions, progress: &impl Progress) -> OpsResult<IngestResult>
```

1. `resolve_wave_name(repo, options.wave.as_deref())` — reuse from `ops/messages.rs`
2. `main_repo_root(repo)` — wave dir lives in main repo, not worktree
3. List `.md` files in `wave/<wave>/`, skip `README.md`
4. Parse numeric prefix (`02-mac-mini-dogfood.md` → prefix=2, slug=`mac-mini-dogfood`)
5. Lowest prefix: one match → copy to `scratch/`, remove original. Multiple → error. None → error.

### Modify: `rust/loopflow/src/ops/mod.rs`
### Modify: `rust/loopflow/src/lf/mod.rs` — add `OpsCommand::Ingest`
### Modify: `rust/loopflow/src/lf/commands/ops/mod.rs` — wire handler
### Modify: `.lf/steps/ingest.md` + builtin copy — add `fast-path: lf ops ingest`

## Verification

```bash
cargo clippy -- -D warnings
cargo test -p loopflow ingest
```
