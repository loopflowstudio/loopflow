# Tend Flow Steps

## Problem

The tend flow and its five steps exist as builtins — prompts and YAML definitions are written — but no tend cycle has actually run end-to-end. scan-waves describes reading git/gh output but doesn't mention the lfd HTTP API, which already exposes wave status, active runs, PR state, queue blocks, and activation history via `lfq show <wave> --json`. The branch routing mechanism works for ship-roadmap but hasn't been validated for tend. The flow test suite covers ops items at the top level but not inside branch sub-flows.

The redesign chord-wave is registered and points at four member wave directories. Everything is in place for the first tend cycle except the wiring.

## Approach

Wire scan-waves to lfd runtime state by updating the step prompt. Add a Rust flow test for branch sub-flows containing ops items. Run the first tend cycle against the redesign chord-wave. Defer the `lf ops` → `lf op` rename — it's 187+ occurrences across the codebase and orthogonal to making tend work.

### 1. Update scan-waves to read lfd state

Add a workflow step to scan-waves.md between "Read wave configs" and "Read recent activity":

```
2. **Read runtime state.** For each member wave:
   - `lfq show <wave-name> --json` — status, iteration, active_run
   - Active run fields: status, step_index, branch, PR state, queue_role, queue_block_reason
   - If no lfd wave exists for a member directory, note it (wave defined on disk but not registered)
```

This uses the existing `lfq` CLI. No code changes — `lfq show --json` already returns `WaveDto` with embedded `WaveRunDto`, `PullRequestDto`, queue state, and triggers.

Data available through this path:
- Wave status (idle/running/waiting/paused/failed), iteration count
- Active run status, current step, PR URL/number/state, draft flag
- Queue role (ready/draft/blocked), block reason (missing_pr/wave_running/rebase_conflict/etc.)
- Trigger configuration, activation history
- Stack position, open PR count

Also update scan-waves to incorporate this in its output template — add a "Runtime" subsection per wave alongside Config, Progress, Items, Blocks, and Open PRs.

### 2. Add flow test for ops in branch sub-flows

The existing `validate_branch_paths` allows `ConcreteItem::Op` in branch paths (flow.rs:759). The builtin `ship-roadmap-build` flow contains `ops: land --create-pr` and is referenced inside `ship-roadmap`'s branch construct. Add a test that loads `ship-roadmap`, expands the branch sub-flow, and asserts the ops item is present.

```rust
#[test]
fn builtin_branch_subflow_contains_ops() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    let flow = load_flow("ship-roadmap-build", repo).unwrap();
    let items = expand_flow(&flow, repo).unwrap();
    assert!(items.iter().any(|item| matches!(item, ConcreteItem::Op(_))));
}
```

### 3. Validate tend flow loads and expands

Add a test that loads the `tend` builtin flow and validates:
- It has 3 items: scan-waves step, assess step, branch
- The branch has two paths: chord (→ tend-chord flow) and reorg (→ reorg flow)
- tend-chord sub-flow expands to 3 steps: draft-chord, review-chord, apply-chord
- reorg sub-flow expands to 1 step: update-wave

```rust
#[test]
fn builtin_tend_flow_structure() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    let flow = load_flow("tend", repo).unwrap();
    assert_eq!(flow.items.len(), 3);
    // ... validate branch paths, sub-flow expansion
}
```

### 4. Refine step prompts for first cycle

Minor prompt refinements based on what the first cycle will actually encounter:

**scan-waves.md**: Add `lfq show` workflow step. Add "Runtime" section to output template. Clarify that member wave names are derived from the chord-wave's area paths (`wave/<name>/` → wave name is `<name>`).

**assess.md**: No changes needed — it reads scratch/tend-scan.md which will now include runtime state.

**draft-chord.md**: No changes needed — mutation levers reference wave YAML fields which are correct.

**review-chord.md**: This step is interactive. On the first tend cycle, the redesign chord-wave runs in manual mode, so this will be a human review session. No changes needed.

**apply-chord.md**: References `lf ops update-wave` for syncing lfd state. This command exists and works. No changes needed.

### 5. Scope out the rename

`lf ops` → `lf op` touches 187+ occurrences across .md files, plus YAML flows (`ops: land`), Rust CLI (`OpsCommand`), docs, golden tests, and step prompts. This is mechanical but wide. It should be a separate wave item — doing it alongside the tend wiring adds noise to the diff and makes the first tend cycle harder to review.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Build a dedicated tend HTTP endpoint that aggregates member wave state | Clean API, single call | Over-engineering — lfq already returns everything scan-waves needs. Agents parse JSON fine. |
| Have scan-waves call raw `curl` against lfd | Faster, no lfq dependency | Port discovery is fragile. lfq handles connection details. |
| Wire tend to a Python orchestrator instead of agent steps | Programmatic control over branch routing | Defeats the purpose — tend is an agent flow, judgment lives in the prompts. |
| Include the ops→op rename in this milestone | Consistency with `ConcreteItem::Op` | 187+ occurrences, orthogonal to tend working. Separate item. |

## Key decisions

**lfq is the interface, not raw HTTP.** Agents executing scan-waves have shell access and lfq installed. `lfq show <wave> --json` returns the full `WaveDto` including active run, PR state, and queue blocks. No new code needed — just prompt guidance.

**Defer the rename.** `lf ops` → `lf op` is the right move but it's a wide mechanical change. The tend flow references `lf ops update-wave` in apply-chord.md. That works today. Rename it when it's the only thing in the diff.

**Test the flow structure, not the runtime.** The Rust flow tests validate parsing and expansion — they can't test agent routing decisions. The first real tend cycle against the redesign chord-wave is the integration test. The Rust tests catch structural regressions (missing steps, broken branch paths, ops items rejected from sub-flows).

**Wave names from area paths.** The chord-wave's area is `wave/chord-model/`, `wave/clear-the-deck/`, etc. The member wave name is the directory name. scan-waves should document this convention explicitly rather than assuming the agent figures it out.

## Scope

- In scope:
  - Update scan-waves.md to read lfd state via lfq
  - Add Rust flow tests for tend flow structure and ops-in-branch-subflows
  - Minor prompt refinements for first-cycle readiness
  - Run first tend cycle against redesign chord-wave (manual validation)

- Out of scope:
  - `lf ops` → `lf op` rename (separate item)
  - Letta integration (item 03)
  - New HTTP endpoints or Python API changes
  - Automated tend scheduling (cron/loop mode)

## Done when

- scan-waves.md includes `lfq show --json` in its workflow and output template
- `cargo test flow_tests` passes with new tend-flow and ops-in-branch tests
- `lf tend` runs against the redesign chord-wave — scan produces a scan doc, assess produces an assessment, branch routes to either chord or reorg
- First real chord is drafted, reviewed, and applied (or: assess finds no pressure points and routes to reorg — either outcome validates the flow)
