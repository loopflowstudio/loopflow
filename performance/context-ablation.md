# Which launch context changes outcomes

```bash
# Weigh every source across recorded launches. No provider spend.
uv run python -m scripts.context_ablation census --since 2026-10-01T00:00:00Z

# Replay one recorded step without one source, in a disposable Machine and checkout.
cargo build -p loopflow --bin lf
uv run python -m scripts.context_ablation arms run_b3b7dc529faa438ba98e4953738241e3
uv run python -m scripts.context_ablation replay run_b3b7dc529faa438ba98e4953738241e3 \
  no-memory --out /tmp/lf-ablation

# Compare arms with the baseline replay, a repeat baseline and the original turn.
uv run python -m scripts.context_ablation report --out /tmp/lf-ablation \
  --repeat /tmp/lf-ablation-repeat
```

Jack Heart requested this study on 2026-09-30 (LOO-349): "we should be doing a
lot more to understand and track our context usage." It asks which implicit
launch sources change what an agent does, and has LOO-346's budgets adopt the
answer.

## Verdicts (2026-10-01)

| Source | Verdict | Budget | Evidence |
|---|---|---|---|
| Wave memory | **Keep** | `memory_tokens` stays 16,000 | Half of a current launch (6.2k median), under a tenth of a turn. Three replays without it changed nothing measurable, and two of the three agents read `MEMORY.md` from disk instead. |
| The Task's own scratch notes | **Keep** | inside `scratch_tokens` | 12.4k tokens at p90. With only these notes inline, all 5 loop-decide replays reached their baseline's verdict. |
| Older scratch | **Trim** | `scratch_tokens` 16,000 → 12,000 | 74–77% of all launch tokens before budgets. Removing every inline note left 4 of 5 loop-decide verdicts unchanged, the same rate at which identical replays disagree. Agents opened the files they needed. |
| Steers older than the last completion | **Drop** (already dropped by #1362) | none | Present in 3 of 95 launches, 147 tokens median. Too rare to replay. |
| Agent-authored comments | **Drop** (already dropped by #1362) | none | Present in 0 of 95 launches. |
| Resumed history | **Unmeasured** | none | Every replayable launch is a fresh turn and `lf replay` never resumes. |

These verdicts come from a pilot: 31 replays of 8 recorded steps. Outcome
measures are size, verdict, checks and time. No one judged whether a diff was
*correct*, so "nothing measurable changed" does not mean "equally good".

## Census: what launches carry

95 replayable implement, compress and loop-decide launches recorded between
2026-08-23 and 2026-10-01. Token weights are the record's own captured asset
weights. The [saved aggregate](baselines/context-census-2026-10-01.json) has
every source.

| Step | Launches | Submitted median / p90 | Older scratch | Own scratch | Wave memory | Instructions |
|---|---:|---:|---:|---:|---:|---:|
| implement, all | 44 | 20,408 / 201,122 | 73.8% | 9.5% | 7.6% | 5.9% |
| compress, all | 46 | 21,950 / 198,821 | 77.2% | 7.9% | 6.4% | 5.7% |
| loop-decide, all | 5 | 24,167 / 26,762 | 41.4% | 48.4% | none | 8.3% |
| implement, since Oct 1 | 12 | 12,012 / 17,108 | 12.0% | 2.8% | 51.2% | 20.9% |
| compress, since Oct 1 | 10 | 12,508 / 16,574 | 10.2% | 8.5% | 52.3% | 15.1% |

Shares are of all submitted tokens in the row. "Own scratch" is the note named
for the Task's worktree plus `questions.md`; everything else under `scratch/`
is "older".

- Before budgets, scratch that was not the Task's own design was three quarters
  of everything launches submitted. One Task (`main-view-task`) accounts for
  2.08M of those tokens.
- Since 2026-10-01 a launch is about 12k tokens and Wave memory is half of it.
- A launch is small beside the turn it starts. Codex implement turns peak at
  157k input tokens (median, n=17); a current 12k launch is under a tenth of that. The
  rest is what the agent reads while working. Claude records carry cost only,
  so their peak input is unavailable.
- Steers weigh 147 tokens at the median in the three launches that have them.

## Replays: what changes without a source

Each arm replays the recorded request with one source edited out. Every arm
gets the same checkout, including the prompt's scratch files on disk, so the
scratch arms measure the inline copy that the budget controls. The
[saved aggregate](baselines/context-ablation-2026-10-01.json) has every row.

### loop-decide (Codex, 5 records, 20 replays)

| Record | Original | Baseline | Repeat baseline | Own scratch only | No scratch |
|---|---|---|---|---|---|
| `c37c40e5` | advance | advance | advance | advance | advance |
| `be5c5e5b` | blocked | blocked | blocked | blocked | blocked |
| `4872be83`* | iterate | iterate | iterate | iterate | iterate |
| `84d7ffb3`* | iterate | iterate | iterate | iterate | **blocked** |
| `51bfced0`* | iterate | **blocked** | iterate | blocked | blocked |

\* Launch commit approximated from the PR head; see Limits.

Peak input fell from about 53k to 40k tokens without inline scratch and to
about 47k with only the Task's notes. Turn time stayed between 0.6 and 2.2
minutes in every arm. Without inline scratch, agents spent more of their
commands reading `scratch/` from disk (5 of 6 against 3 of 8 in one record).

One identical pair of baselines disagreed, so one flipped verdict in five is
the noise floor, and `no-scratch` sits on it.

### implement and compress (Claude, 3 records, 11 replays)

| Record | Arm | Minutes | Cost | Checks | Files | Lines | File overlap with baseline |
|---|---|---:|---:|---:|---:|---:|---:|
| implement `b3b7dc52` | original | 4.7 | $1.53 | 1 | unavailable | unavailable | unavailable |
| | baseline | 3.5 | $1.17 | 1 | 2 | 226 | 1.00 |
| | repeat baseline | 8.2 | $1.44 | 4 | 5 | 294 | 0.40 |
| | no memory | 3.8 | $1.18 | 1 | 4 | 268 | 0.50 |
| | no scratch | 4.9 | $1.43 | 1 | 5 | 294 | 0.40 |
| implement `b07bc6d4` | original | 9.8 | $3.05 | 1 | unavailable | unavailable | unavailable |
| | baseline | 11.7 | $4.05 | 7 | 16 | 853 | 1.00 |
| | no memory | 12.9 | $4.32 | 6 | 21 | 823 | 0.48 |
| | memory trimmed to 4k | 10.3 | $4.49 | 6 | 18 | 1,001 | 0.62 |
| compress `61725520` | original | 2.1 | $0.74 | 3 | unavailable | unavailable | unavailable |
| | baseline | 3.9 | $0.98 | 1 | 5 | 243 | 1.00 |
| | repeat baseline | 3.7 | $0.83 | 1 | 4 | 113 | 0.80 |
| | no memory | 5.2 | $0.91 | 2 | 4 | 135 | 0.80 |
| | no scratch | 2.5 | $0.74 | 2 | 3 | 78 | 0.33 |

Two identical implement replays shared 40% of their changed files and differed
2.3× in time. Every ablated arm falls inside that spread. Compress without
inline scratch produced the smallest change and the lowest overlap (0.33
against 0.80 for its repeat); at n=1 that is a reason to look again, not a
finding.

Removing memory saved 5–8k launch tokens and no measurable time or cost. In
`b07bc6d4` and `61725520` the agent then read `MEMORY.md` itself.

## What the budgets adopt

- `memory_tokens` stays 16,000. Memory is the largest launch source today and
  nothing here shows it is dead weight; agents fetch it when it is missing.
- `scratch_tokens` drops from 16,000 to 12,000 and `scratch_bytes` from 128 to
  96 KiB. That covers the Task's own notes at p90 and spends nothing on older
  notes, which stay on disk behind the excerpt's pointer.
- `goal_tokens` and `input_tokens` are unchanged. Launch messages weigh 2k
  tokens at p90, and submitted input 17k since budgets, against limits of
  16,000 and 64,000.

The larger cost is not in the launch. A turn reads ten times its launch
context while working, and resumed history sits outside what a replay can
remove. Those need their own instrument.

## Limits

- **Pilot size.** One to five records per cell, all from this repository. The
  noise floor is as large as any difference observed.
- **Outcome, not quality.** Verdict, changed files, checks, time and cost are
  compared. Whether a change honoured a decision recorded in Wave memory was
  not judged.
- **Launch commit.** Manifests do not record HEAD. The collector reads the
  source worktree's reflog. Three loop-decide records had lost their worktree;
  their commit is the last PR #1354 head commit before launch, and one of them
  blocked in every arm on a checkout that contradicted its notes. Uncommitted
  state at launch is unrecoverable.
- **Environment.** A replay has no Task, Flow, remote or Linear. `lf flow`
  verdict commands fail there, so the verdict is the one the agent tried to
  record. Replays ran a branch build of lf from 2026-10-01 against records
  written by lf 0.12.28, cloned without a warm build cache.
- **Resumed history, old steers and agent comments** were not replayed.
- **Cost** is the provider's reported figure for Claude turns ($21.57 across
  11 replays). Codex replays report tokens, not cost.

## Extending the cohort

`replay` needs an lf that keeps its store in `LF_HOME`; lf 0.12.28 does not,
and nested `lf` calls inside its replays reach the real store. Build this
checkout's lf, or use a release that includes #1386. The collector stages the
variant record, clones the repository with no remote, links provider logins,
and admits the recorded login to the disposable Machine.

Add records before trusting a verdict: several implement and compress steps
per Wave, a repeat baseline for each, and a reviewer's judgment of whether
the ablated diff is still right.
