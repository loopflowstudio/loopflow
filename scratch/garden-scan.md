# Tend Scan - 2026-07-06

## Wave: meta
### Config
Primary flow: `ship-roadmap`; mode: `manual`; workers: `0`.

Intent: make loopflow's own agent runs sharp by measuring what prompts loaded,
what commands ran, what spawned, and what it cost. Telemetry is local-only.

Metrics emphasize reconstructable local runs, measured prompt changes, token
reduction with quality held, paved-road compliance, builtin flow legibility, and
the general/taste split.

### Runtime
No `wave/meta/.wave-endpoint` file exists in this worktree. `tmux ls` is blocked
by sandbox permissions: `Operation not permitted`. Live roadmap reads are blocked
because `lf op pm show --wave <name>` cannot decrypt the Asana token through the
macOS keychain in this sandbox.

GitHub PR state is unavailable from this sandbox: `gh pr list` cannot connect to
`api.github.com`.

### Progress
The current worktree is clean on branch `jack-heart.bugs.20260705_1627.meta`.
Recent local ledger state exists under `.lf/journal/runs/8985c55b-9864-4c2b-860f-b7054a71bbea/`.

The local run ledger shows a Meta-relevant defect: concurrent `lf op pm show`
commands wrote interleaved bytes into `events.jsonl`, producing malformed JSONL.
This contradicts the "every run is reconstructable locally" metric.

### Items
Live Asana roadmap unavailable due keychain access failure.

| # | Title | Status |
|---|-------|--------|
| 01 | Local run ledger reconstruction | blocked by malformed concurrent JSONL writes |
| 02 | Asana roadmap scan | blocked by keychain permission in this sandbox |
| 03 | Prompt/run measurement improvements | queued by Meta goal, no live roadmap available |

### Blocks
- Asana token decryption fails in this sandbox, so live roadmaps cannot be read.
- GitHub API is unavailable, so open PR and CI status cannot be verified.
- tmux socket access is denied, so live worker/wave sessions cannot be listed.
- Local run ledger has malformed JSONL from concurrent writes.

### Open PRs
Unavailable: GitHub API connection failed.

### Unlanded Branches
- `jack-heart.architecture.20260705_1756`: 1 commit ahead of `main`; extracts
  dispatch helpers from `lfd` into `rust/loopflow/src/dispatch.rs`; diff is
  530 insertions / 526 deletions across 7 files. Matches Architecture's
  collapse goal.
- `jack-heart.systems.20260705_1712`: 3 commits ahead of `main`; release-note
  CI fallback and review artifacts; diff is 161 insertions / 10 deletions across
  8 files.
- `jack-heart.bugs.20260705_1627.goals`: committed branch has a launch probe
  compression; its worktree also has substantial uncommitted changes touching
  dispatch extraction, harness conformance tests, and Swift wave content parser
  deletion: 303 insertions / 781 deletions plus new files.
- `jack-heart.wave-roster-tidy.20260705_1800`: pushed branch deleting local
  `root`, `mobile`, and `workflows` wave surfaces while adding Concerto's Asana
  project mapping; diff is 3 insertions / 162 deletions across 7 files.
- Several architecture run worktrees remain ahead of the old
  `origin/jack-heart.architecture.20260704_1707` branch.

## Wave: architecture
### Config
Primary flow: `build`. Goal: collapse `lf`, `lfd`, and `lfq` toward one
workhorse plus one thin subscription server; net-negative code each pass.

### Runtime
No live runtime state verified; tmux and PR state unavailable.

### Progress
Architecture has recent unlanded work matching its goal:
`jack-heart.architecture.20260705_1756` moves dispatch behavior out of
`lfd::executor::helpers` into shared `dispatch`.

### Items
Live Asana roadmap unavailable.

| # | Title | Status |
|---|-------|--------|
| 01 | Dispatch extraction from lfd | in-flight |
| 02 | Session record spine proposal | in-flight by memory, not approved |

### Blocks
PR/CI status unavailable; several older architecture run worktrees remain.

## Wave: goals
### Config
Primary flow: `ship-roadmap`. Goal: turn waves into persistent looping agents
against goals, steered by live Asana roadmaps.

### Runtime
No live runtime state verified; tmux and PR state unavailable.

### Progress
Memory says the reactive server and M1/M2 compression have shipped or are open.
The local goals worktree contains significant uncommitted work crossing dispatch,
harness conformance, and Swift parser deletion.

### Items
Live Asana roadmap unavailable.

| # | Title | Status |
|---|-------|--------|
| 01 | Reconcile roadmap into Asana | blocked by auth/keychain |
| 02 | Prove-the-language reference builds | in-flight |
| 03 | M1 charter extraction | in-flight |

### Blocks
The dirty goals worktree needs triage before more work is layered onto it.

## Wave: systems
### Config
Primary flow: `ship-roadmap`; mode: `manual`; workers: `0`. Goal: keep CI,
release, freshness, host, and automation boring.

### Runtime
No live runtime state verified; tmux and PR state unavailable.

### Progress
Systems memory records shipped `lf op reset-waves` and deterministic rebase /
worktree placement. Branch `jack-heart.systems.20260705_1712` is ahead of main
with release-note CI fallback work.

### Items
Live Asana roadmap unavailable.

| # | Title | Status |
|---|-------|--------|
| 01 | Release fallback / PR review artifacts | in-flight |
| 02 | Cadenza release parity | queued by memory |
| 03 | Cron host bootstrap | queued by memory |

### Blocks
PR/CI status unavailable; main worktree has unrelated local memory/token files.

## Wave: concerto
### Config
Primary flow: `ship-roadmap`; mode: `manual`; workers: `0`. Goal: Concerto
frames vendor terminal sessions; it does not render chat.

### Runtime
No live runtime state verified; tmux and PR state unavailable.

### Progress
Memory records the terminal-first rebuild, A2 `Wave.repo -> repos:[RepoWork]`,
and the ghost-session reclaim invariant. The roster-tidy branch adds Concerto's
Asana project mapping.

### Items
Live Asana roadmap unavailable.

| # | Title | Status |
|---|-------|--------|
| 01 | Terminal-first rebuild | in-flight by memory |
| 02 | Ghost-session reclaim invariant | folded into memory |
| 03 | Asana project mapping | in-flight on roster-tidy branch |

### Blocks
PR/CI status unavailable.

## Wave: website
### Config
Primary flow: `ship-roadmap`; mode: `manual`; workers: `0`. Goal: public site
and docs live from one source in `docs/`.

### Runtime
No live runtime state verified.

### Progress
Memory records site import and deploy from `website/`. Next work is content,
style, docs nav, and deploy smoke coverage.

### Items
Live Asana roadmap unavailable.

| # | Title | Status |
|---|-------|--------|
| 01 | Content and style alignment | queued by memory |
| 02 | Deploy smoke follow-ups | queued by memory |

### Blocks
No website-specific runtime state verified.

## Wave: root
### Config
Primary flow: `garden`; mode: `manual`; workers: `0`. Goal: conduct active
waves and keep status language coherent.

### Runtime
No live runtime state verified.

### Progress
Local files still define root as an active conductor wave, but
`jack-heart.wave-roster-tidy.20260705_1800` deletes the root local wave surface.

### Items
Live Asana roadmap unavailable.

| # | Title | Status |
|---|-------|--------|
| 01 | Active-wave roster cleanup | in-flight on roster-tidy branch |

### Blocks
Root's active/retired status is inconsistent between current local files and the
roster-tidy branch.

## Wave: workflows
### Config
Primary flow: `ship-roadmap`; mode: `manual`; workers: `0`. Goal: scheduling,
providers, flow execution, mutation, and governance surfaces.

### Runtime
No live runtime state verified.

### Progress
Current local files still define workflows as active. Roster-tidy deletes the
local workflows GOAL.

### Items
Live Asana roadmap unavailable.

| # | Title | Status |
|---|-------|--------|
| 01 | Active-wave roster cleanup | in-flight on roster-tidy branch |

### Blocks
Workflows' active/retired status is inconsistent between current local files and
the roster-tidy branch.

## Wave: mobile
### Config
Archived. Goal states there is no active mobile surface and no work should be
invented without an explicit decision.

### Runtime
No live runtime state verified.

### Progress
Mobile is archived in current local files. Roster-tidy removes the local mobile
GOAL entirely.

### Items
No active items; live Asana roadmap unavailable.

### Blocks
None, if archived status is intentional.

## Cross-Wave
- `wave-roster-tidy` changes the wave map itself: root/mobile/workflows local
  surfaces disappear, while Concerto gets an Asana mapping. Until landed or
  rejected, garden scans will disagree about which waves are active.
- Architecture and Goals both have dispatch-related work in play. The clean
  Architecture branch and the dirty Goals worktree touch overlapping dispatch
  surfaces; sequencing matters.
- Systems and Concerto now share the ghost-session reclaim invariant.
- Meta's ledger corruption is not only a Meta bug: it undermines every wave's
  ability to reconstruct dispatched work.

## Raw Signals
- `uv run` is blocked in this sandbox by access to `~/.cache/uv/sdists-v9/.git`.
  Direct `lf` invocation works far enough to expose Asana keychain failure.
- The active branch's `.lf/metrics/ops.jsonl` only records two `wt.create` events.
- Main worktree has local dirty memory/token files outside this worktree.
