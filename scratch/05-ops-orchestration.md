# Ops Orchestration

## Problem

Flows mix agent judgment with mechanical operations. The `land` step in `ship-roadmap` spins up an agent session just to run `lf ops land --title "..." --body "..."`. The `release` step wraps a single `lf ops release run` call. These agent sessions cost 30-60 seconds of startup time and token budget for work that's either (a) mechanical execution or (b) a small judgment call followed by mechanical execution.

Meanwhile, the judgment that *does* matter — PR copy and release notes — is either passed as CLI args (fragile, no caching) or generated mechanically from PR titles (low quality).

The people who benefit: every wave that runs `build → gate → land`. That's most of them.

## Approach

Split the problem into two layers:

**Layer 1: OpsItem in flows.** Add `ops:` as a first-class flow item type. The flow runner calls Rust functions directly — no agent, no startup cost. This replaces agent-wrapper steps for mechanical operations.

**Layer 2: File-based handoff for judgment.** Agent steps write judgment artifacts to `scratch/`. Ops items read them. The interface is files, not CLI args.

### Flow YAML syntax

```yaml
# ship-roadmap becomes:
- ingest
- kickoff
- review-design
- build
- review
- ops: land          # reads scratch/pr-title.txt + scratch/pr-body.md
```

```yaml
# Ops items with args:
- ops: land --create-pr
- ops: rebase
- ops: release run patch
- ops: pr
```

The `ops:` prefix maps directly to `lf ops <command>` subcommands. Args are space-split strings — same grammar as the CLI.

### PR copy via gate step

The existing `gate` step already inspects the branch and writes a review doc. Extend it to also write `scratch/pr-title.txt` and `scratch/pr-body.md`. This means PR copy is generated as a side effect of the quality gate — no separate step needed.

When `ops: land` executes, it reads these files. If they exist, it uses them as `--title`/`--body`. If they don't exist (e.g., manual `lf ops land` without running gate first), current behavior is preserved — `--title`/`--body` remain required CLI args.

### Staleness check

`gate` writes `scratch/.pr-copy-ref` containing the HEAD SHA at generation time. `ops: land` compares this to current HEAD. If they match, use cached copy. If HEAD moved, the flow re-runs gate (or land falls back to requiring CLI args for manual use).

### Release notes via step

`release run` gains a hook point: after bumping manifests and before committing, it writes PR data to a temp file and invokes `lf release-notes` as a subprocess. The `release-notes` step reads PR data + previous RELEASE_NOTES.md and writes narrative notes.

This is ops-calls-agent, but scoped to a single judgment call within a deterministic procedure. The step is customizable via `.lf/steps/release-notes.md`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep land as an agent step | Simple, no new FlowItem variant | 30-60s startup cost per land, tokens spent on mechanical work. Every `ship-roadmap` run pays this tax. |
| Ops invokes agent for all judgment (land-copy + release-notes + PR update) | Single command does everything | Reintroduces the `ops/agent.rs` coupling that was deliberately deleted. Ops becomes dependent on agent runtime. |
| Separate `land-copy` step before `ops: land` | Explicit two-step: judgment then mechanics | Extra step in every flow. Gate already does this work — adding a second step that reads the same diff is redundant. |
| Move PR copy to `.lf/pr/` instead of `scratch/` | Survives scratch clear | Adds a new state directory. scratch/ clear already happens *after* reading PR copy — sequencing solves this without new directories. |

## Key decisions

**`ops:` items are strings, not structs.** The YAML syntax is `ops: land --create-pr`, not `ops: { command: land, args: [--create-pr] }`. Simpler to write, simpler to parse (shell-split the string), and mirrors how users think about these commands.

**Gate writes PR copy, not a separate step.** The gate step already reads the diff and forms opinions. Writing PR copy there means one agent session handles both quality check and PR authoring. Users who want different PR conventions override `.lf/steps/gate.md`.

**scratch/ clear happens after PR copy is consumed.** `ops: land` reads `scratch/pr-title.txt` and `scratch/pr-body.md`, then clears scratch/ as part of the land procedure. This resolves the open question from the wave item without introducing new directories.

**Release notes use subprocess invocation.** `release_run` calls `lf release-notes` via `Command::new`. This is the minimal coupling point — ops doesn't import agent internals, it shells out to a step. The step gets full loopflow context (repo docs, direction, area).

**ConcreteItem::Ops variant, not ConcreteItem::Step.** The expanded flow distinguishes ops items from agent steps at the type level. This means the flow runner, the pipeline header, and the Swift/Python visualizations can all treat ops items differently (e.g., no elapsed time counter, different icon).

## Scope

### In scope

- `FlowItem::Ops(OpsItem)` variant with parsing, expansion, and execution
- `FlowAction::RunOps` variant in next_action()
- CLI flow runner (`lf/commands/flow.rs`) executes ops items via existing ops functions
- Daemon flow runner (`lfd/executor/wave/mod.rs`) executes ops items via existing ops functions
- `gate` step extended to write `scratch/pr-title.txt` + `scratch/pr-body.md` + `scratch/.pr-copy-ref`
- `ops: land` reads PR copy files, clears scratch after consuming them
- `release_run` invokes `lf release-notes` subprocess for narrative notes
- `release-notes` builtin step (receives PR data, writes RELEASE_NOTES.md)
- Python model: add `ops` type to flow step representation
- Swift model: display ops items in FlowProgressPills with distinct styling
- Update builtin flows: `ship-roadmap`, `ship`, `deploy`, `build` to use `ops:` items where appropriate
- Tests: flow parsing, expansion, execution for ops items

### Out of scope

- Procedure customization via `.lf/config.yaml` (stretch item — defer)
- `tag_and_push_ref` with non-HEAD target_ref unit test (noted gap, not blocking)
- Fast-path for ops items in daemon (ops items are already fast)

## Done when

```bash
# Ops item parses from YAML
cargo test -p loopflow flow_tests  # includes ops parsing tests

# Gate writes PR copy
lf gate  # produces scratch/pr-title.txt, scratch/pr-body.md, scratch/.pr-copy-ref

# Ops land reads PR copy
lf ops land  # without --title/--body, reads from scratch/ files

# ship-roadmap uses ops: land
cat rust/loopflow/src/engine/builtins/flows/code/ship-roadmap.yaml
# last item is "ops: land", not "land" step

# Release notes are narrative
lf ops release run patch  # RELEASE_NOTES.md has themed sections, not just PR titles

# Flow visualization shows ops items
# FlowProgressPills renders ops items with ops-specific styling
swift test --package-path swift

# All CI passes
cargo fmt --check && cargo clippy -- -D warnings && cargo test --all
uv run pytest python/tests/
swift test --package-path swift
```

Wave goals advanced: "flows use `ops:` items directly without agent wrappers" and "`lf ops land` and `lf ops release run` invoke agent steps for judgment."
