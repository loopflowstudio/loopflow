# Release Cadence: Decomposed Ops + Step + Cron

## Problem

`lf ops release` is a monolithic 2000-line function that does everything: check for changes, bump versions, generate notes, land PRs, tag, and monitor workflows. This works for manual releases but blocks two things:

1. **Agent-driven releases.** The `lf release` step needs granular commands to handle each phase — checking for changes, bumping, noting, tagging, monitoring. A monolith gives no seams for this.

2. **Automated cadence.** Daily patch and monthly minor releases via cron need the step, which needs the decomposed commands. Without decomposition, cron just re-runs the manual workflow blindly.

## Approach

Three layers, bottom-up:

### 1. Decompose `lf ops release` into focused commands

Extract five public functions from `release.rs` and wire them as sibling CLI subcommands:

```
lf ops release-check    [--target T]                    # exit 0 if PRs merged since last tag
lf ops release-notes    <version> [--prev-tag TAG] [-t]  # generate notes (LLM-powered)
lf ops release-bump     <version> [--target T]           # bump manifests
lf ops release-tag      <version> [--target T]           # tag and push
lf ops release-status   [--target T]                     # check CI
```

**Keep `lf ops release <version>` working.** The monolithic command stays as the happy path for manual use. Internally, refactor it to call the decomposed functions. The decomposed commands are for the step prompt and power users.

**CLI structure:** `ReleaseCheck`, `ReleaseNotes`, `ReleaseBump`, `ReleaseTag`, `ReleaseStatus` are sibling `OpsCommand` variants alongside the existing `Release`. This avoids nested subcommand ambiguity with the positional version arg (`lf ops release patch` already means "release with patch bump").

```rust
enum OpsCommand {
    Release { version, dry_run, target, status },  // existing, unchanged
    ReleaseCheck { target },                         // new
    ReleaseNotes { version, prev_tag, target },      // new
    ReleaseBump { version, target },                 // new
    ReleaseTag { version, target },                  // new
    ReleaseStatus { target },                        // new (mirrors --status flag)
}
```

Clap auto-converts `ReleaseCheck` to `release-check` at the CLI level.

### 2. `lf release` step prompt

A step prompt at `.lf/steps/release.md` (updating the existing one) that orchestrates the decomposed commands:

```
1. lf ops release-check → skip if nothing merged
2. lf ops release-bump <version>
3. lf ops release-notes <version>
4. lf ops commit -m "release: v<version>"
5. lf ops land
6. lf ops release-tag <version>
7. lf ops release-status → verify workflow passed
```

The step accepts an optional version as message text: `lf release` (defaults to patch), `lf release minor`, `lf release major`. The human (or cron config) decides the version, not the agent. The agent executes the release mechanically, handling failures and re-entry at each phase.

Also update the builtin step prompt at `rust/loopflow/src/engine/builtins/steps/ops/release.md` to match.

### 3. Cadence wave configs

Two wave configs in `wave/` targeting the loopflow repo itself as first consumer:

```yaml
# wave/release-patch/release-patch.yaml
flow: release
message: patch
stimulus:
  kind: cron
  cron: "0 2 * * *"    # 2 AM daily

# wave/release-minor/release-minor.yaml
flow: release
message: minor
stimulus:
  kind: cron
  cron: "0 2 1 * *"    # 2 AM, 1st of month
```

Both use the same `release` flow (single step: `release`). The wave config passes the version as message text. Daily runs produce patches, monthly runs produce minors. The agent doesn't decide version — it executes what it's told, skipping if nothing merged.

Create a `release` flow definition:

```yaml
# .lf/flows/release.yaml
steps:
  - release
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Nested subcommands (`lf ops release check`) | Cleaner hierarchy | Ambiguous with positional `version` arg in clap; `lf ops release patch` could mean "run release with patch" or "run release's patch subcommand" |
| Replace monolith entirely | Forces decomposed-only path | Monolith has robust resume logic, error handling. Breaking it loses a tested, working orchestration. |
| Agent decides version | More autonomous | Version selection is a product decision, not a judgment call. Humans/cron configs specify patch/minor/major explicitly. Default to patch. |
| Include Concerto config UI | Full feature | ~400 LOC of Swift for config editing. "Release Now" button is useful; config editing is premature — cron waves handle cadence. Defer config UI to follow-up. |

## Key decisions

**Sibling variants, not nested subcommands.** `release-check`, `release-notes`, `release-bump`, `release-tag`, `release-status` are OpsCommand variants alongside `Release`. This avoids the clap ambiguity between `lf ops release patch` (version arg) and `lf ops release <subcommand>`. The hyphenated form (`release-check`) is conventional for related-but-distinct commands. `release-status` gets its own variant for consistency even though `--status` flag exists — the step prompt uses `lf ops release-status`, not `lf ops release --status`.

**Monolith stays.** `lf ops release patch` continues to work. It's refactored internally to call the same public functions the decomposed commands use, but the orchestration logic (resume, bootstrap, worktree management) remains. The step prompt doesn't use the monolith — it calls individual commands with judgment between each.

**Version is input, not judgment.** The version comes from the human (`lf release minor`) or the wave config (`message: patch`). No version provided defaults to patch. The agent's judgment is spent on release notes quality (analyzing merged PRs for user-facing impact), not on version selection.

**`lf` available in all agent environments.** The step prompt tells agents to run `lf ops release-check` etc. This requires `lf` on PATH. Today it's available in Concerto (bundled + symlinked), the lfd container (`/usr/local/bin/`), and local installs (`~/.local/bin/`). The agent container (`docker/agent/`) is missing it. Install `lf` in the agent container Dockerfile so any step that calls `lf ops` works in containerized environments.

**`release-check` outputs structured data.** `release_check()` returns `Vec<MergedPr>` internally, but the CLI command should print structured output (PR number, title, files changed) so the agent has context for writing release notes. JSON when stdout is not a TTY, human-readable table otherwise.

**Step prompt handles re-entry.** If a release fails mid-way and the step runs again, each command is idempotent: bumping an already-bumped manifest is a no-op, tagging an existing tag is detected. The step prompt should check what's already done before re-running phases — specifically, check if the version commit is already on main before re-bumping.

**Concerto deferred to follow-up.** The ops decomposition + step + waves is a complete milestone that delivers automated releases. Concerto "Release Now" button and config UI are a natural follow-up but not needed for the cadence to work.

## Scope

- In scope:
  - Extract `release_check()`, `release_notes()`, `release_bump()`, `release_tag()` as public functions in `release.rs`
  - Wire 5 new OpsCommand variants to CLI (`release-check`, `release-notes`, `release-bump`, `release-tag`, `release-status`)
  - Structured output from `release-check` (JSON when piped, human-readable otherwise)
  - Install `lf` in agent container (`docker/agent/`)
  - Update step prompt to use decomposed commands, with re-entry handling
  - Update builtin step prompt
  - Create wave configs for daily patch + monthly minor
  - Create `release` flow definition
  - Tests for new public functions
  - Tests for new CLI subcommands

- Out of scope:
  - Concerto UI (follow-up wave)
  - `format_pr_for_prompt()` tests (nice-to-have, not blocking)
  - RELEASE_NOTES.md truncation for long histories (release frequency solves this)
  - Cross-repo release coordination

## Done when

```bash
# Decomposed commands work independently
lf ops release-check                # exit 0 → changes exist; prints PR list
lf ops release-check | jq           # JSON output when piped
lf ops release-check                # exit 1 → nothing merged, skip
lf ops release-notes 0.9.6          # generates RELEASE_NOTES.md
lf ops release-bump 0.9.6           # bumps Cargo.toml, pyproject.toml
lf ops release-tag 0.9.6            # tags and pushes
lf ops release-status               # reports workflow status

# Monolith still works
lf ops release patch                # full workflow, as before

# Step orchestrates mechanically
lf release                          # defaults to patch, runs full release
lf release minor                    # explicit minor
lf release                          # nothing merged → skips cleanly

# lf available in agent container
docker run loopflow-agent lf --version   # lf is on PATH

# Wave configs exist for cron
cat wave/release-patch/release-patch.yaml   # daily cron
cat wave/release-minor/release-minor.yaml   # monthly cron

# Tests pass
cargo test -p loopflow release
```

## Implementation sketch

### release.rs changes

```rust
// New public functions (extracted from existing private code):

/// Check if any PRs have merged since the last tag.
/// Returns Ok(prs) if changes exist, Ok(empty vec) if not.
pub fn release_check(repo: &Path, target_name: Option<&str>) -> OpsResult<Vec<MergedPr>> { ... }

/// Generate release notes for the given version.
/// Writes RELEASE_NOTES.md. Returns the generated notes content.
pub fn release_notes(
    repo: &Path,
    version: &str,
    prev_tag: Option<&str>,
    target_name: Option<&str>,
    progress: &impl Progress,
) -> OpsResult<String> { ... }

/// Bump version in all manifest files for the target.
pub fn release_bump(
    repo: &Path,
    version: &str,
    target_name: Option<&str>,
    progress: &impl Progress,
) -> OpsResult<()> { ... }

/// Create a git tag and push it to the remote.
pub fn release_tag(
    repo: &Path,
    version: &str,
    target_name: Option<&str>,
) -> OpsResult<String> { ... }
```

`MergedPr` becomes public. `publish_release()` refactored to call these.

### CLI wiring (lf/mod.rs)

```rust
#[command(name = "release-check")]
ReleaseCheck {
    #[arg(short = 't', long = "target")]
    target: Option<String>,
},
#[command(name = "release-notes")]
ReleaseNotes {
    version: String,
    #[arg(long = "prev-tag")]
    prev_tag: Option<String>,
    #[arg(short = 't', long = "target")]
    target: Option<String>,
},
#[command(name = "release-bump")]
ReleaseBump {
    version: String,
    #[arg(short = 't', long = "target")]
    target: Option<String>,
},
#[command(name = "release-tag")]
ReleaseTag {
    version: String,
    #[arg(short = 't', long = "target")]
    target: Option<String>,
},
#[command(name = "release-status")]
ReleaseStatus {
    #[arg(short = 't', long = "target")]
    target: Option<String>,
},
```

### Step prompt update

The release step becomes the orchestrator that calls decomposed ops commands in sequence. Version is input (default: patch). The agent's judgment is spent on release notes — analyzing merged PRs for user-facing impact, grouping changes, writing scannable summaries. The mechanical phases (check, bump, tag, status) are executed without judgment.

### Agent container changes

Install `lf` in `docker/agent/Dockerfile` so steps that call `lf ops` commands work in containerized environments. Add to `install-loopflow.sh` or directly in the Dockerfile:

```dockerfile
COPY --from=builder /build/target/release/lf /usr/local/bin/lf
```

Or install from release:

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
```
