# 05: Ops Orchestration

**Finish line:** `lf ops land` and `lf ops release run` invoke agent steps for judgment (PR copy, release notes) and flows use `ops:` items directly without agent wrappers.

## Context

`lf ops release run` shipped as a deterministic orchestrator — one Rust function owns the full release lifecycle. But the broader pattern (ops orchestrate, steps provide judgment) isn't complete. Land still requires the agent to compose and pass `--title`/`--body`. Flows still wrap mechanical ops in agent steps. Release notes are still mechanical PR lists.

Design doc: `5e2ebef6` deleted agent-invocation code from ops. This sprint restores agent invocation at controlled points — ops calls the agent as a subroutine, not the other way around.

## Key principle: idempotent steps, not state inspection

Each phase of a procedure should be safe to re-run. Don't build state-inspection code to figure out where a previous attempt left off — make tag creation tolerate existing tags, make publish tolerate already-published versions, make PR creation tolerate existing PRs.

## Prior work

`release_tag` idempotency is now tested: same-commit re-tag succeeds, different-commit re-tag fails. Test helpers `git_output` and `git_output_bare` verify both local and remote state. Gap: `tag_and_push_ref` with a non-HEAD `target_ref` (used by `release_run` when tagging a merged commit) is exercised by the full flow but not unit-tested in isolation.

## Items

### OpsItem in flows

Add `Ops(OpsItem)` variant to `FlowItem` enum. Flow runner calls the corresponding Rust function directly — no agent session, no startup cost.

```rust
struct OpsItem {
    command: String,     // "land", "rebase", "release run"
    args: Vec<String>,
}
```

Changes: `FlowItem` enum, flow YAML parser, `expand_flow()` passthrough, `next_action()` returns `Action::RunOps`, flow runner in both `lf` and `lfd`, Python/Swift model updates for flow visualization.

### Land-copy step

`lf ops land` invokes a `land-copy` step that writes `scratch/pr-title.txt` + `scratch/pr-body.md`. Ops reads these files, clears scratch, creates/updates PR.

Users override `.lf/steps/land-copy.md` to change PR conventions without touching the mechanical procedure.

### Staleness check for PR copy

Track commit SHA at which PR copy was last generated (`scratch/.pr-copy-ref`). On land: if HEAD matches, reuse copy. If HEAD moved, re-run the step. Avoids paying agent startup cost when the branch hasn't changed.

### Narrative release notes

`lf ops release run` invokes a `release-notes` step that writes `RELEASE_NOTES.md` with themed sections and narrative descriptions. The step receives merged PR data, previous release notes (for voice continuity), and the version string.

This restores capability that existed at `7429125c` and was lost when agent-invocation code was deleted from ops.

### Procedure customization (stretch)

Mechanical sequences configurable via `.lf/config.yaml`:

```yaml
land:
  strategy: squash
  auto_merge: true
  draft: false
  scratch: clear

release:
  targets:
    default:
      tag_prefix: "v"
      manifests: ["Cargo.toml", "pyproject.toml"]
      workflow: "release.yml"
```

## Open question

**scratch/ lifecycle during land.** Currently `lf ops land` clears scratch/ before landing. PR copy lives in scratch/. Options: read PR copy then clear, exclude PR copy from clear, or move PR copy outside scratch/ (e.g. `.lf/pr/`).
