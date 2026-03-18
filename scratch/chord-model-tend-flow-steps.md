---
asana_id: '1213718081081138'
linear_id: 70cde070-1b10-4e97-87b0-e72d35e50d7d
---
# 02: Tend Flow Steps — Live Proof

## Problem

The tend flow is structurally complete — YAML, steps, router, WaveDto, flow tests — but has never run against live lfd state. Until it does, the chord model's central coordination loop is a blueprint, not a machine. This item closes the gap between "it parses" and "it works."

Who benefits: anyone running `lf tend` against a chord-wave — the mechanism that lets a parent wave observe, judge, and tune its member waves. The redesign chord is the first customer.

Why now: every subsequent chord-model item (algedonic signals, self-healing heuristics, VSM flow) assumes tend works. Proving it now prevents building on an untested foundation.

## Approach

One script. `scripts/tend-demo.py` boots an isolated lfd, registers the redesign chord and its members, runs `lf tend`, and captures every artifact. The script is the operating recipe and the proof in one artifact.

### What changes

**1. Fix `scripts/bootstrap-redesign.py`** — currently creates waves but never configures redesign with `primary_flow="tend"` or its member wave area. After creation, call `update_wave` to set:
- `primary_flow: "tend"`
- `area: ["wave/chord-model/", "wave/signals/"]`

Also update `WAVE_NAMES` to drop `clear-the-deck` and `agent-embedding` (those waves were folded into chord-model; signals remains as the other member).

**2. Create `scripts/tend-demo.py`** — single entry point for the live proof:

```
1. Build lfd (cargo build --bin lfd)
2. Start lfd in isolated LF_HOME (temp dir, ephemeral port — reuse LfdRuntime pattern)
3. Point lfd at the real repo (not a temp repo — tend needs wave/ dirs on disk)
4. Run bootstrap-redesign.py against the isolated lfd
5. Verify: lfq show redesign --json returns live state with flow=tend
6. Verify: lfq show chord-model --json, lfq show signals --json return live state
7. Run lf tend for redesign (headless, via lf CLI pointed at isolated lfd)
8. Capture scratch/tend-scan.md, scratch/route-or.md, scratch/tend-assessment.md
9. Print summary: which path was chosen, what artifacts were written
10. Stop lfd, clean up
```

**3. LF_HOME isolation** — the demo uses `LfdRuntime` but with the real repo instead of a temp git repo. This means:
- `HOME` env var → temp dir (isolates `~/.lf/session-token` and db)
- `LFD_HTTP_ADDR` → ephemeral port (doesn't collide with running lfd)
- Repo path → current worktree (tend steps need `wave/`, `scratch/`, git history)

The `LfdRuntime` class already does the first two. The change: pass the real repo path to wave creation instead of `runtime.repo_dir`.

**4. Extend LfdRuntime for real-repo use** — add an optional `use_repo` parameter. When set, skip `_initialize_git_repo` and point wave creation at the provided repo. The temp HOME still isolates auth/db state.

### Running both paths

First run against a quiet chord should route to `silence` — no open PRs, no failing CI, no stalled items. The script captures this.

For the `tune` exercise: the script creates a synthetic pressure point before the second run. Concretely, it writes a temporary wave item file to `wave/chord-model/99-pressure-test.md` with content indicating a stalled item. This gives scan-waves something to flag, and assess a reason to route to `tune`. After the run, the script removes the pressure file.

If the agent environment doesn't support running full `lf tend` end-to-end in headless mode within the demo (no agent API key, model unavailable), the script falls back to documenting the manual recipe and verifying the pre-conditions (lfd boots, waves register, lfq show returns expected state).

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Shell script (`tend-demo.sh`) | Simpler, no Python dependency | Can't reuse LfdRuntime, harder to manage lfd lifecycle and API calls, less composable |
| Manual recipe in scratch/ | No code to maintain | Fragile, drifts from reality, nobody runs it twice |
| E2E test in pytest | Runs in CI | Tend needs agent API keys and models, which CI doesn't have; demo script can be run locally when ready |
| Docker-based isolation | Fully hermetic | Over-engineered for a dev proof; LF_HOME isolation is sufficient |

## Key decisions

**Reuse LfdRuntime, don't reinvent.** The E2E test infrastructure already solves isolated lfd lifecycle. Extending it to accept a real repo is a small change that keeps the demo trustworthy.

**Python, not shell.** The bootstrap script and loopflow API are Python. The demo needs to call both, manage lfd process lifecycle, and handle errors. Python is the right tool.

**Pressure point via temporary file, not API manipulation.** Scan-waves reads wave directories from disk. A temporary item file is the most natural way to create pressure that scan-waves will see. Mutating wave status via API would only affect runtime state, which assess reads secondarily.

**One script, not a flow.** This is a proving tool, not a production workflow. It should be runnable with `uv run python scripts/tend-demo.py` and produce a clear pass/fail with captured artifacts.

**Real repo, isolated daemon.** The tend steps need real `wave/` directories, real git history, real scratch/ space. Only the daemon state (db, tokens, port) needs isolation.

## Scope

- In scope:
  - Fix bootstrap-redesign.py to configure redesign wave properly
  - Extend LfdRuntime to support real-repo mode
  - Create tend-demo.py that boots, bootstraps, runs tend, captures artifacts
  - Exercise silence path (quiet chord) and document tune path prerequisites
  - Capture artifacts in scratch/ for reviewer inspection

- Out of scope:
  - `lf ops` → `lf op` rename (separate item)
  - Algedonic signal integration (depends on tend working first)
  - CI integration of tend-demo (needs agent keys)
  - VSM flow as a separate flow type (later item)
  - Stall detection and self-healing heuristics

## Done when

```bash
# All of these pass:
uv run python scripts/tend-demo.py          # boots lfd, bootstraps, runs tend, captures artifacts
uv run pytest python/tests/                  # bootstrap changes don't break existing tests
cargo test --all                             # no Rust regressions
```

Observable outcomes:
- `lfq show redesign --json` returns a wave with `flow: tend` and `area` pointing at member wave dirs
- `scratch/tend-scan.md` exists with lfd-backed runtime data for each member wave
- `scratch/route-or.md` exists with a valid path selection
- If silence: script confirms quiet-chord routing was correct
- If tune: `scratch/tend-assessment.md` and `scratch/tend-chord.md` exist with assessment and mutations
- Script prints a summary a reviewer can read to confirm the cycle completed

## Wave alignment

**Vision** — "Every chord is a viable system." This item proves the first chord's central coordination loop actually runs, moving from structural to operational viability.

**Goals** — Advances: "VSM flow cycles that produce at least one actionable change: >50%" — can't measure this until tend runs at all.

**Risks** — "VSM steps could become formulaic checklists instead of genuine system assessment" — the pressure-point exercise specifically tests whether assess produces real judgment vs boilerplate. If the first live run reveals formulaic output, that's a signal to revise the assess step prompt before building further.
