# Ops Orchestration — Design Review

## What was implemented

Added `ops:` as a first-class flow item type so flows can execute mechanical operations (land, rebase, release) directly via Rust functions instead of spinning up agent sessions. Extended gate to write PR copy files for handoff. Added narrative release notes via subprocess agent invocation.

**Four changes, end to end:**

1. **`FlowItem::Ops(OpsItem)` + full pipeline** — parsing from YAML (`ops: land --create-pr`), expansion to `ConcreteItem::Ops`, `FlowAction::RunOps` in `next_action()`, execution in both CLI (`lf/commands/flow.rs`) and daemon (`lfd/executor/wave/mod.rs`) flow runners.

2. **PR copy handoff via gate → land** — `gate` step extended to write `scratch/pr-title.txt`, `scratch/pr-body.md`, and `scratch/.pr-copy-ref` (HEAD SHA for staleness check). `lf ops land` reads these files, validates freshness, uses them for PR creation, then clears scratch/.

3. **Narrative release notes** — `release_run` invokes `lf release-notes` as a subprocess, passing merged PR data + previous release notes via a temp file (`LF_RELEASE_NOTES_CONTEXT` env var). New `release-notes` builtin step.

4. **Visualization** — Python `FlowStep.from_raw()` handles `ops:` prefixed strings. Swift `FlowProgressPills` shows ops items with distinct cyan styling and an "ops" badge.

## Key choices

- **Ops items are strings, not structs** in YAML. `ops: land --create-pr` instead of `ops: { command: land, args: [--create-pr] }`. Shell-split parsing matches how users think about CLI commands.

- **Gate writes PR copy, not a separate step.** Gate already reads the diff and forms opinions — writing PR copy there means one agent session handles both quality check and PR authoring.

- **`execute_flow_ops` routes through clap parsing.** The ops item string is reconstructed as `["lf", "ops", command, ...args]` and parsed through the existing `Cli::try_parse_from`. This reuses all existing validation and option handling without duplicating match arms.

- **Scratch clear happens after PR copy is consumed.** `land()` calls `resolve_pr_copy()` before `clear_scratch()`. This resolves the open question from the wave item without introducing new directories.

- **Fork branches reject ops items.** Ops items are mechanical — running them in parallel fork branches doesn't make sense and could cause conflicts.

## How it fits together

```
Flow YAML → parse_ops_value → FlowItem::Ops(OpsItem)
  → expand_with_chain → ConcreteItem::Ops(ConcreteOps)
  → next_action → FlowAction::RunOps
  → execute_flow_ops → Cli::try_parse_from → execute_parsed_ops → existing ops functions
```

Gate writes artifacts to scratch/. Land reads them. The interface between agent judgment and mechanical execution is files on disk.

## Risks and bottlenecks

- **Clap parsing overhead**: `execute_flow_ops` reconstructs argv and parses through clap. This is negligible for ops (sub-millisecond) but is an unusual pattern — future ops subcommands need to be compatible with both CLI and flow-runner invocation paths.

- **`lf` binary must be on PATH for release notes**: `run_release_notes_step` shells out to `lf --batch release-notes`. If `lf` isn't installed or is a different version, this fails. Mitigated by using `Command::new("lf")` which follows PATH.

- **Staleness check is SHA-exact**: If gate runs, then a trivial commit is added (e.g., formatting), the PR copy is invalidated even though content is essentially the same. This is conservative and correct — better to regenerate than use stale copy.

## What's not included

- Procedure customization via `.lf/config.yaml` (stretch item, deferred)
- `tag_and_push_ref` with non-HEAD `target_ref` unit test (noted gap, not blocking)
- `deploy` flow update — left unchanged since it uses `gate → update-wave`, not land
