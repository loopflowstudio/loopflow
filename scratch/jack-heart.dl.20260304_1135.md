# Ops as orchestrator, ops in flows

## Problem

Three related issues:

1. **Agent-as-orchestrator is unreliable.** Land, release, and commit steps have the agent call `lf ops` commands. The agent wanders, takes exploratory steps before acting, and — in land's case — calls `lf ops land` which rotates the worktree out from under the still-running agent.

2. **Steps are the only flow-composable unit.** Flows can only contain steps (agent sessions). Mechanical ops like land and rebase require an agent wrapper step even when no judgment is needed, paying startup cost every time.

3. **Customization is all-or-nothing.** Overriding `.lf/steps/land.md` changes both the PR copy conventions (voice/judgment) and the mechanical procedure (push, rebase, auto-merge). These are different concerns with different customization shapes.

## Design

### Principle: ops orchestrate, steps provide judgment

Ops commands own deterministic procedures. When a procedure needs judgment (PR copy, release notes, conflict resolution), the ops command invokes a step at that phase. The step writes files. The ops command reads them and continues.

The agent is a subroutine of the procedure, not the orchestrator.

### Principle: filesystem as contract

Steps produce judgment by writing files to known paths. Ops commands consume those files. No structured output parsing, no JSON, no stdout capture. The agent has filesystem access — let it write files.

### Principle: judgment is cached, not repeated

PR copy is expensive (agent startup). Once generated, it's valid until the branch moves. Ops commands check staleness (new commits since last generation) and only re-invoke the agent when the copy is out of date.

If the branch hasn't changed since the last successful land, skip the agent entirely.

### Principle: idempotent steps, not state inspection

Each phase of a procedure should be safe to re-run. Don't build elaborate state-inspection code to figure out where a previous attempt left off — make tag creation tolerate existing tags, make publish tolerate already-published versions, make PR creation tolerate existing PRs. (Lesson from release.rs: the resume-from-state approach added ~300 lines that were eventually deleted in favor of idempotency.)

## Land

### Current

```
agent (outer loop)
├── git log, git diff (understand branch)
├── compose PR title + body
└── lf ops land --title "..." --body "..." --create-pr
    ├── commit dirty changes
    ├── rebase
    ├── clear scratch/
    ├── create/update PR
    ├── enable auto-merge
    └── rotate worktree  ← agent's cwd disappears
```

### Proposed

```
lf ops land (outer loop, deterministic)
├── commit dirty changes
├── rebase
├── check: is PR copy stale? (new commits since last generation?)
│   ├── yes → run "land-copy" step (agent writes scratch/pr-title.txt + scratch/pr-body.md)
│   └── no  → reuse existing copy
├── read scratch/pr-title.txt + scratch/pr-body.md
├── clear scratch/ (after reading PR copy)
├── create/update PR with title + body
├── enable auto-merge
└── rotate worktree  ← no agent alive, safe
```

### Judgment step: land-copy

The "land-copy" step is the customizable part. It defines PR title conventions, body structure, what context to pull from. Users override `.lf/steps/land-copy.md` to change how copy is written, without touching the procedure.

The step receives the branch context (diff, commit log, scratch/ contents) and writes:
- `scratch/pr-title.txt` — one line
- `scratch/pr-body.md` — markdown body

### Staleness check

Track the commit SHA at which PR copy was last generated. Store alongside the copy (e.g. `scratch/.pr-copy-ref`). On land:
- HEAD matches stored ref → reuse copy
- HEAD has moved → re-run the step

### Procedure customization (future)

The mechanical sequence is configurable via `.lf/config.yaml`, not by overriding the step:

```yaml
land:
  strategy: squash        # squash | merge | rebase
  auto_merge: true
  draft: false
  scratch: clear          # clear | keep
```

Not built now. Designed for.

### Open question: scratch/ lifecycle

Currently `lf ops land` clears scratch/ before landing. PR copy lives in scratch/. Options:
- Land reads PR copy *then* clears scratch/
- PR copy files are excluded from the clear
- PR copy lives outside scratch/ (e.g. `.lf/pr/`)

## Release

### History

The release system has been rewritten several times. Key lessons:

- **Narrative release notes matter.** Mechanical PR lists aren't good enough. Agent-generated narrative notes (themed sections, real descriptions of what shipped) existed at `7429125c` and were lost when all agent-invocation code was deleted from ops in `5e2ebef6`.
- **Idempotency beats state inspection.** An explicit resume system (`try_resume_release`, ~300 lines) was added and later deleted. The current approach — tolerant tag creation, `skip-existing` on publish, `workflow_dispatch` for re-runs — is simpler and more robust.
- **Orchestration keeps getting rewritten.** release.rs oscillated between monolithic (`publish_release`), decomposed (5 subcommands + agent orchestrator), and re-unified (`release_run`). The subcommands are useful for debugging but the top-level orchestrator should be one function.
- **CI constrains the design.** GITHUB_TOKEN tag pushes don't trigger workflows. The auto-tag workflow must explicitly dispatch the release workflow. Publish steps must tolerate already-published versions for re-runs.

### Current

```
agent (minimal — just calls one command)
└── lf ops release run <version>
    ├── check merged PRs
    ├── create release worktree
    ├── bump manifests
    ├── generate mechanical release notes (PR list, no narrative)
    ├── commit + PR + land
    ├── wait for merge
    ├── tag merged commit
    ├── cleanup worktree
    └── wait for release workflow
```

### Proposed

```
lf ops release run <version>
├── check merged PRs since previous tag
├── create release worktree
├── bump manifests
├── run "release-notes" step
│   agent writes RELEASE_NOTES.md in worktree
│   receives: merged PR data, previous notes (for voice continuity), version
├── commit release changes
├── create PR + land (calls lf ops land internally)
│   └── land invokes "land-copy" step for the release PR title/body
├── wait for merge queue completion
├── tag merged commit
├── cleanup release worktree
└── wait for release workflow / GitHub Release
```

Two agent invocations: one for release notes, one for PR copy (via land). Both write files, both are subroutines of the deterministic procedure.

### Subcommands stay

`release check`, `release bump`, `release notes`, `release tag`, `release status` remain individually callable for debugging and manual recovery. `release run` calls them internally.

### Procedure customization (future)

```yaml
release:
  targets:
    default:
      tag_prefix: "v"
      manifests: ["Cargo.toml", "pyproject.toml"]
      workflow: "release.yml"        # already exists
```

## Ops in flows

### Current flow items

```rust
enum FlowItem {
    Step(Step),
    Fork { branches },
    FlowRef(String),
    Branch(BranchDef),
}
```

### Proposed

```rust
enum FlowItem {
    Step(Step),
    Ops(OpsItem),           // new
    Fork { branches },
    FlowRef(String),
    Branch(BranchDef),
}

struct OpsItem {
    command: String,         // e.g. "land", "rebase", "release run"
    args: Vec<String>,       // e.g. ["--local", "patch"]
}
```

### Flow YAML

```yaml
# flow: build
- implement
- compress
- lint
- gate
- ops: land

# flow: ship
- design
- implement
- gate
- ops: land

# flow: integrate
- ops: rebase

# flow: deploy
- gate
- ops: release run patch
```

### Flow runner changes

Both `lf` CLI (`flow.rs`) and `lfd` daemon (`wave/mod.rs`) handle `OpsItem`:
- Parse the command string to an `OpsCommand`
- Call the corresponding Rust function directly
- No agent session, no prompt, no startup cost
- Ops that internally invoke steps (land → land-copy, release → release-notes) handle that themselves
- Report progress through the existing `Progress` trait

### Expansion

`expand_flow()` / `expand_with_chain()` pass `OpsItem` through as-is. No expansion needed.

`next_action()` returns a new `Action::RunOps(OpsItem)` variant.

### UI impact

Python and Swift models represent ops items in flow visualization:
- Flow chain shows ops as a distinct item type
- Step progress tracking works the same — step_index advances past ops items

## Rebase

Rebase already works well as a fast-path. No changes to the pattern.

When rebase appears in a flow as `ops: rebase`, it runs mechanically. If it fails (conflicts), the flow runner fails the flow. The wave or user handles recovery.

Standalone `lf rebase` keeps the fast-path behavior: mechanical rebase, agent on conflict.

## Migration path

1. Add `OpsItem` to `FlowItem` enum, update parsers and flow runners
2. Restore narrative release notes: `lf ops release notes` invokes a step that writes RELEASE_NOTES.md
3. Rewrite `lf ops land` to invoke a "land-copy" step for PR title/body
4. Add staleness check for PR copy
5. Update builtin flows to use `ops: land` and `ops: rebase` instead of wrapper steps
6. Keep standalone `lf land` and `lf rebase` working (they call `lf ops land` / `lf ops rebase`)

## What doesn't change

- `lf land` (standalone) still works
- `lf rebase` still works as fast-path
- Steps that are pure judgment (design, review, implement) are unaffected
- Flows that don't use ops commands are unaffected
- Python and Swift remain thin clients — all execution in Rust
- The decomposed release subcommands stay for debugging
