# Finishing the work

Status measured on 2026-07-09 against the real ledger and a real build, not
inferred from the diff. Every claim below was run.

Read this first; the other scratch docs are the detail:

| Doc | What it is |
|---|---|
| `telemetry.md` | The audit. Why the ledger could not be trusted. |
| `telemetry-fix.md` | The contract and the pass that implemented it. |
| `dashboard-design.md` | The consumer. What each chart demands of the ledger. |
| `evals-design.md` | Is loopflow worth it, provably. Not started. |
| `questions.md` | Forks that are the maintainer's, not the implementer's. |

## Where things stand

**Shipped and verified.**

- Span identity (`process_id` / `parent_process_id`), closed vocabularies,
  absolute `repo`, `model`, cumulative usage. `f78e856c`.
- `lf doctor` exits 0 on the real ledger. All six checks green.
- `lf trace` prints a real process tree with per-span cost and model, and the
  children's costs sum to the run total.
- `lf runs` summarizes per process, not per trace.
- `lf wavechat` — one pane that steers and monitors a wave. `f848f0bb`.
- 1029 Rust tests, clippy, fmt, and `swift build` all green.

**Written but never exercised.** This is the whole of the remaining risk.

- The three charts have never rendered real data. `TelemetryDashboardView.swift`
  compiles; nobody has opened it.
- The cost waterfall has never been reconciled against `lf usage`.
- The silence ribbon has never been shown a real gap.

**Not started.** The evals harvester. The PR delivery record. `escalated`.

## Do this next, in order

### 1. Finish the dashboard slice (the only thing blocking "done")

`telemetry-fix.md` done-whens 8, 9, and 10. They are the reason the pass exists;
schema without a chart is no product value.

Build and open the app (see "Demo" below), then:

- **Flamechart.** Open a real trace. Nesting must match `lf trace <id>` exactly.
  An `open` span must render hatched, at its last known timestamp — never to
  "now", never dropped. Trace `db8a254f` has one (a killed run), so this is
  testable today.
- **Cost waterfall.** Its bars must sum, bar for bar, to what `lf usage` reports
  for that run. **If they disagree, `own_spend`'s cumulative-diff rule is wrong
  and the pass is not done.** This is the assertion that keeps the charts honest.
- **Silence ribbon.** Verify it by making a real gap: point `LF_HOME` at a
  scratch dir, record nothing for a while, and confirm the ribbon goes black for
  that window instead of interpolating across it.

Screenshot each into the PR.

### 2. Steering verbs for `lf wavechat`

The wave-chat KR promises the human can *interrupt bad runs*. Today `/status` is
the only non-speech verb, so a bad run can be watched and talked to but not
stopped. Add `/pause`, `/resume`, `/interrupt` against the wave's doors. Keep
the rule: a slash command is a steering verb, everything else is speech.

### 3. The evals harvester (slice 1)

`evals-design.md`. Zero LLM spend, and it builds the corpus while everything
else proceeds. Target cadenza first — `server/tests/*.py` is pure Python and the
construction validated there in half a second. The validation rule is not "the
tests fail"; it is *collection succeeds, the pre-existing tests pass, and the new
tests fail*. A task that errors at import is not a task, it is a broken
environment wearing a task's clothes.

### 4. What the dashboard's remaining metrics need

Neither is telemetry work; both are their own pass. Do not synthesize either
from `runs.snapshot_pr` — 7 of 171 rows have one, all from `break-test`.

- **A delivery record** (run/wave → PR number → merge event). Unblocks landed
  PRs, cost per landed PR, and lead time.
- **An `escalated` event that fires.** `LfEventType::Escalated` is defined and
  emitted zero times. Human-intervention counts have no source until it does.

## Demo

### The telemetry dashboard

`RegistryQueryLocal` shells out to `lf runs --json`, `lf trace <id> --json`, and
`lf doctor --json`, so the **new** `lf` must be on `PATH`. The installed
`~/.local/bin/lf` is older than this branch and has none of those flags.

```bash
# 1. new lf on PATH
cargo build --bin lf
export PATH="$PWD/target/debug:$PATH"

# 2. the ledger answers for itself — expect exit 0, six green checks
lf doctor

# 3. a trace with real spans, nesting, cost, model, and an `open` frame
lf runs
lf trace db8a254f          # or any id from `lf runs`

# 4. the app. NOT `swift run` — that builds a bare executable, and
#    NotificationService dies on `bundleProxyForCurrentProcess is nil`.
cd swift && xcodegen generate
xcodebuild -project LoopflowSwift.xcodeproj -scheme LoopflowMac \
  -configuration Debug -destination 'platform=macOS' \
  CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO build
open ~/Library/Developer/Xcode/DerivedData/LoopflowSwift-*/Build/Products/Debug/Loopflow.app
```

Then **Go → Telemetry (⌘1)**. Four cards: run flamechart, cost waterfall,
cache-hit ratio, ledger silence.

The app must inherit the same `PATH`, so launch it from that shell (`open` does).
If the charts are empty, `lf doctor --json` from the app's environment is the
first thing to check.

### `lf wavechat`

Two panes. One serves the wave, one steers it.

```bash
# pane 1 — a listener with no resident flowloop, so it spends no tokens
lf wave intelligence --no-flowloop

# pane 2
lf wavechat intelligence
```

The session opens by replaying the thread's recent history, then live events
scroll past. Type to speak into the thread; the wave's own event stream echoes
what it received. `/status` reads the health door, `/help` lists the verbs,
`/quit` or Ctrl-D leaves — the wave keeps running.

Drop `--no-flowloop` for the real thing: the resident flowloop wakes on what you
type, and its turns scroll past in the same pane.

## One gotcha, which is the design working

A shell launched *by* an `lf` run inherits `LF_RUN_ID`, so every `lf` invoked
from it joins that trace as a child span. That is correct — it is what makes
`lf trace` show nested work. It also means a demo run from such a shell will not
mint a fresh trace. Prefix `env -u LF_RUN_ID` when you want a root.
