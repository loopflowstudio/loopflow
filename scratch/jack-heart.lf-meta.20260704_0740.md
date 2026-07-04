# Demoable observability: `lf runs` + `lf trace`

## What to build

One command that shows everything loopflow did on this machine, and one that
reconstructs a single run.

## The demo

After a day of normal loopflow use:

```
$ lf runs
TIME   REPO      WAVE  RUN            DURATION  TOKENS  STATUS
09:14  loopflow  meta  code           12m       184k    ok
09:41  cadenza   —     implement      6m        90k     ok
10:02  loopflow  meta  gate           3m        41k     error
$ lf trace <run-id>
code  (12m, 184k tokens)                    jack-heart.meta.20260704
├─ implement   6m   90k   prompt: ~/.lf/logs/loopflow/meta/…implement.md
├─ compress    2m   31k   prompt: …compress.md
├─ lint        1m   12k
└─ gate        3m   51k   error: merge gate red — see output log
```

Every number local, cross-repo (loopflow and cadenza side by side), zero
network. The developer answers "what ran, in what order, what did it cost,
where did it go wrong" in two commands.

## Data map (what already exists)

- `.lf/journal/runs/<run_id>/events.jsonl` — lifecycle events (Started/
  Completed/Errored per run/flow/step, ts, command, wave, worktree). Wave
  worktrees only today.
- `~/.lf/lfd.db` — `runs` (started/ended, parent_run_id, flow_parents),
  `run_token_usage` (input/output/cache per run_id, repo column),
  `terminal_sessions` (argv, parent_session_id).
- `~/.lf/logs/<repo>/<worktree>/*.md` — assembled prompt + context per step
  (run_id in filename when LF_RUN_ID is set — landed this branch).
- `~/.lf/output/<run_id>.log` — parsed stream output.

## Gaps to close (in scope)

1. **Plain CLI runs are invisible.** Journaling is wave-worktree-only and CLI
   runs mint no run_id. Fix: mint a run_id for every `lf` invocation; write
   the same `events.jsonl` lifecycle events for all runs.
2. **Journal is per-worktree; reads must be machine-grain** (decided: analysis
   sees every repo on the machine). Fix: journal writes ALSO go durable under
   `~/.lf/logs/<repo>/<worktree>/events.jsonl` — same dual-write pattern the
   prompt logs already use, same directory tree, so one walk serves both.
3. **CLI runs record no tokens/duration.** `StreamEvent::Result { cost_usd,
   duration_secs }` and `StreamEvent::Usage` are parsed then dropped in the
   CLI path. Fix: fold them into the run's Completed event.

## Key functions

```rust
// lf/commands/runs.rs
pub fn run(trace: Option<&str>) -> Result<()>;      // `lf runs` / `lf trace <id>`
fn collect_runs(since: Duration) -> Vec<RunRecord>;  // walk ~/.lf/logs/**/events.jsonl + lfd.db
fn render_timeline(runs: &[RunRecord]);              // grouped, most recent first
fn render_trace(run: &RunRecord);                    // step tree + prompt paths + tokens
```

`RunRecord`: run_id, repo, worktree, wave, flow/step tree with per-node
start/end, tokens (joined from run_token_usage or stream events), prompt-log
paths (globbed by run_id), output-log path, status.

## Constraints

- Local-only, always. No network reads, no phone-home. (Wave hard rule.)
- Read paths must tolerate partial data: runs before this change have no
  run_id in prompt filenames, no events for CLI runs — degrade to what exists,
  never error.
- Don't touch the lfd HTTP DTO surface in v1 — this is a CLI reader over
  files + local db. (DTO changes mean 3-language fixtures; defer.)
- events.jsonl stays append-only; new fields are additive.

## Done when

`lf runs` after `lf : "say hi"` shows that run with duration and tokens;
`lf trace <id>` shows its prompt-log path. `cargo test` green, clippy clean.

## Measure

Before: reconstructing a day of runs = manual archaeology across 4 sources.
After: two commands. Also gives the wave its first empirical read on metric 1
("every run is reconstructable locally").
