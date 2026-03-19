---
asana_id: '1213718081081138'
linear_id: 70cde070-1b10-4e97-87b0-e72d35e50d7d
---
# Tend Live Proof + VSM Flow

Two deliverables on this branch: fix the bootstrap script (small), build the VSM flow (the real work).

## 1. Bootstrap fix

`scripts/bootstrap-redesign.py` already updated:
- Dropped `clear-the-deck` and `signals` from `WAVE_NAMES` (both folded into chord-model per #580)
- After wave creation, calls `loopflow.update_wave("redesign", flow="tend", area=["wave/chord-model/", "wave/agent-embedding/"])`

This is done. Tests should still pass.

## 2. VSM flow

Five builtin steps and one flow YAML. Each governance level (s5-s2) follows the same pattern: assess, update wave plans, then optionally implement if something is urgent at that altitude. s1 launches a parallel batch.

### Steps to create

All under `rust/loopflow/src/engine/builtins/steps/vsm/`:

**vsm/s5.md** — Identity and Policy
- Reads: wave/<chord>/ (member READMEs, configs, roster), algedonic history
- Writes: scratch/vsm-s5.md (assessment), wave plan updates (if boundary/roster changes needed)
- Assess: Is the chord responsible for the right things? Member roster correct? Autonomy levels appropriate? Direction drifted?
- Implement path: chord boundary is wrong — create/archive/merge/split a wave, correct direction drift

**vsm/s4.md** — Intelligence
- Reads: scratch/vsm-s5.md, environment (deps, APIs, upstream changes)
- Writes: scratch/vsm-s4.md (assessment), wave plan updates (if env changes need items)
- Assess: What changed since last cycle? Upstream impacts? New deps/deprecations? What's coming?
- Implement path: urgent environmental change — breaking dep, security advisory, API sunset

**vsm/s3.md** — Control
- Reads: scratch/vsm-s5.md, scratch/vsm-s4.md, `lfq show <wave> --json` for each member, algedonic history
- Writes: scratch/vsm-s3.md (assessment + batch size recommendation), wave plan updates (reprioritization, resource allocation)
- Assess: Member performance, velocity, error rates. Algedonic history. Blocks. Resource allocation — how many items in the next batch?
- Implement path: mechanical block — failing CI, stalled PR, config error
- s3 determines batch size

**vsm/s2.md** — Coordination
- Reads: scratch/vsm-s3.md, all member wave backlogs, open PRs, area overlap analysis
- Writes: scratch/vsm-s2.md (assessment), scratch/vsm-batch.md (the batch manifest)
- Assess: Overlapping areas? Conflicting PRs? Oscillation? Trigger/dependency changes needed?
- Output: updated backlogs per wave + next batch (items safe to run in parallel)
- Implement path: active interference — conflicting PRs, duplicate work, trigger loops
- Simulates member wave perspectives to reason about conflicts. Letta upgrades this later.

**vsm/s1.md** — Operations
- Reads: scratch/vsm-batch.md
- Launches each batch item as a subwave run: own worktree, own branch, own PR
- Runs through ship-roadmap machinery (ingest → kickoff → build → gate → land)
- Failed runs generate algedonic signals for next cycle's s3

### Flow YAML

`rust/loopflow/src/engine/builtins/flows/vsm/vsm.yaml`:

```yaml
- vsm/s5
- vsm/s4
- vsm/s3
- vsm/s2
- vsm/s1
```

### Batch manifest format

`scratch/vsm-batch.md`:

```markdown
# VSM Batch — <date>

## Batch size
N items (determined by s3 resource assessment)

## Items
| Wave | Item | Why now |
|------|------|---------|
| chord-model | 03-wave-discovery | Unblocks disk-based wave creation |
| agent-embedding | 02-letta-setup | s4 flagged API changes |

## Parallel safety
<s2's reasoning about why these items don't conflict>
```

### The or pattern at each governance level

Each s5-s2 step should end with a routing decision. The step prompt includes:

```
After assessment, decide:
- **continue**: Assessment is sufficient. Update wave plans and move to the next level.
- **implement**: Something at this level is urgent enough to fix now. Implement it, then update wave plans.
```

This isn't a flow-level `or:` — it's a decision within the step. The step either writes assessment + plan updates, or writes assessment + plan updates + code changes. Either way, it writes to scratch/ for the next level.

### s1 architectural notes

s1 is the first step that spawns subwave runs. Implementation options:

1. **lfd API**: s1 calls `lfq run <wave>` for each batch item. lfd manages worktree creation and run lifecycle. Algedonic signals flow naturally.
2. **Direct worktree + lf**: s1 creates worktrees via `lf ops wt create` and runs `lf ship-roadmap` in each. Simpler but bypasses lfd tracking.

Prefer option 1 if lfd supports launching runs for specific items. Fall back to option 2 if not. Either way, s1 should report which runs it launched and their initial status.

### What to test

- Flow YAML parses and expands correctly (Rust flow tests)
- Each step prompt has correct `requires:`/`produces:` frontmatter
- Existing tend flow tests still pass
- `cargo test --all` and `uv run pytest python/tests/` green
