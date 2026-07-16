# Let users drill from roadmap intent to live work and complete trace (W2-122)

## Problem

A roadmap row (Wave / Project / Task) and its execution evidence (runs, traces,
tokens, PRs) live in the same ledger and are already joined by stable string
identifiers — the Linear issue identifier (`W2-122`) and the project slug. But
the join is not reachable from the product surface:

- Every agent launch inside a Task Session is durably stamped with `task`
  (issue identifier) and `project` (slug) — the Intelligence durable trace link
  (`agent_launches.task` / `.project`, indexed).
- `lf runs --json` (`SkillRunEntry`) **drops both fields** in `summarize_runs`.
  A run declares only its `wave`. So no consumer — CLI or Mac — can join a run
  to the roadmap Task that produced it, except by heuristics over `worktree`.
- There is no way to ask "show me this Task's runs." `lf runs` is machine-wide
  or (internally) wave-scoped only.

So a user standing on `W2-122` in `lf roadmap` cannot drill to its runs, and
from a run cannot reach its complete trace on the human surface, because the
run row shows the launch id, which `lf trace` does not accept.

This is the missing spine the whole W2-122 story hangs on: preserve
Wave/Project/Task identity **into runs** and make the drill traversable.

## The demo

```
lf roadmap --wave product        # W2-122 sits in a section (Now / Needs attention / …)
lf runs --task W2-122            # every run W2-122 produced: flow/skill, tokens, cost, status, a trace id
lf trace <trace-id>             # the complete process tree, prompts, artifacts, reason
```

Today step 2 returns nothing joinable and step 3 can't be reached from a run id.
After this slice the three commands traverse one identifier — the Linear
issue identifier — from intent to complete trace.

## Approach

Close the join at the source and make it drillable, CLI-first (the Mac runs
ledger renderer does not exist yet — confirmed — so the contract lands before
its consumer, per the wave's "frame, don't render / CLI is source of truth").

1. **Surface the foreign key on runs.** Add `project: Option<String>` and
   `task: Option<String>` to `SkillRunEntry`, copied from the `AgentLaunchRow`
   that already carries them. Mirror the two fields on the Swift `SkillRunEntry`
   (DTO lockstep). A run now declares which roadmap Project/Task owns it, or
   `null` when it was launched outside a Task/Project session — honest, never
   inferred.

2. **Make runs drillable by identity.** Add `--task <ID>` and `--wave <NAME>`
   filters to `lf runs`. Both filter the same launch set the command already
   reads. `--task` is the roadmap drill; `--wave` promotes the existing internal
   `wave_runs` scoping to a first-class flag. Filtering is one shared reader so
   `list`, the new flags, and `wave_runs` (used by `lf status`) can't diverge.

3. **Close the drill loop to the trace.** `lf trace` today addresses by exec id
   (process id) or trace id (run id) — but `lf runs` shows the *launch* id, which
   `lf trace` rejects. Make `lf trace` also resolve a launch id to its trace, so
   any id a user reads in `lf runs` opens its complete trace. One id, continuous
   drill.

Out of this slice (named, not silently dropped): embedding runs into the
roadmap/status plan tree (machine-wide roadmap must stay a bounded, deterministic
read — runs are fetched on drill, not per-row); the Mac runs-ledger renderer
(consumes this contract next); surfacing `incomplete_reason`/artifacts on the
run summary (the trace already carries them — the drill reaches them).

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Is Wave/Project/Task identity actually preserved into the durable trace? | Yes. `agent_launches` has `project` (slug) + `task` (issue identifier), indexed (`0.11.006_context_launch_work.sql`), stamped by `child_work_attribution` from `LF_TASK_SESSION_ID`/`LF_PROJECT_SESSION_ID`. | The join key exists; the fix is surfacing it, not schema work. |
| Where is it dropped? | `summarize_runs` (`runs.rs:688`) builds `SkillRunEntry` without copying `launch.project`/`launch.task`. | One local change; no new capture path. |
| Can a run be joined without these fields today? | Only by `wave`, or by matching `worktree` strings (the Swift agent's fallback). Fragile. | The foreign key removes the heuristic. |
| Does `lf trace` accept the id `lf runs` shows? | No. `lf runs` shows the launch id; `lf trace` resolves exec id (process id) or trace id (run id) only. | Add launch-id resolution to `trace_id_for_address` so the human drill is continuous. |
| Will adding two Optional fields break the Swift decoder? | No — Codable ignores unknown keys, and adding the fields to the Swift mirror keeps the DTO rule satisfied (required-or-Optional, no defaults). | Mirror both fields as `String?`. |
| Do runs launched outside a Task session have identity? | No — `project`/`task` are `NULL`. | Emit `null`; the roadmap drill simply won't list them, which is correct — they belong to no Task. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Embed `runs: Vec<SkillRunEntry>` in `TaskDetailSnapshot` so `lf status`/`lf roadmap` carry per-task runs | Mac gets runs free from the existing `status()` call | Machine-wide roadmap is documented as a bounded, deterministic, network-free read; embedding every run per task per wave blows that budget. Drill is on-demand, not per-row. |
| Add `project`/`task` columns to `run_events` (span/exec grain) | Process-tree drill could reach Task | Bigger schema change across a shared ledger (migration-collision hazard per wave memory); the launch grain already carries identity and is the grain runs are read at. Not needed for this slice. |
| Change the `lf runs` RUN column to show `trace_id` instead of launch id | `lf trace <shown>` works with no trace change | Splits human vs `--json` identity (json keys on launch `id`); two launches share a trace id so the column would repeat. Resolving launch id in `trace` keeps one identity everywhere. |

## Key decisions

- **The join key is the Linear issue identifier**, the same value that addresses
  `lf task attach <issue>`, `PmItem.identifier`, and `agent_launches.task`. One
  string threads intent → runs → trace → live session → PR. `--task` takes that
  identifier verbatim.
- **Honest nulls over inferred joins.** A run with no `task` is not guessed onto
  a roadmap row by worktree or wave — it reads `null`. The task's "do not infer
  joins by title" is honored.
- **One shared runs reader.** `list`, the filters, and `wave_runs` fold through
  one function taking an optional wave/task filter, so the surfaces can't drift.
- **CLI-first contract.** The wire type gains the field now; the Mac renderer
  consumes it next. Matches the wave's CLI-is-source-of-truth invariant.

## Scope

- In scope: `project`/`task` on `SkillRunEntry` (Rust + Swift mirror);
  `lf runs --task <ID>` and `--wave <NAME>`; `lf trace` accepting a launch id;
  tests for each; help text.
- Out of scope: Mac runs-ledger renderer; embedding runs in the plan tree;
  `run_events`/span Project/Task columns; run-summary error/artifact fields.

## Done when

```
lf runs --task <ID> --json | jq '.[].task'      # each run reports its Task identifier
lf runs --task <ID>                             # human table filtered to that Task
lf trace <launch-id-from-lf-runs>               # opens the complete trace
```

`cargo test -p loopflow` (runs/status integration tests) green; Swift
`SkillRunEntry` carries `project`/`task`; `cargo fmt` + `clippy` clean.

## Wave alignment

Advances Auditability KR "Drill-down holds end to end, every time: wave state ->
run detail -> attachable live session" and "Curation always points back: every
planning claim drills to the raw record" — a roadmap Task now drills to the raw
run and trace by one identifier. Serves the product objective's "inspect its
record" and "without caring which process owns the machinery."
