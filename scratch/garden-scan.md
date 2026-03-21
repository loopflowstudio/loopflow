# Garden Scan — 2026-03-20

## Wave: model

### Config
- **Flow**: build-or-silent
- **Mode**: manual
- **Direction**: clarity, care
- **Area**: rust/loopflow/src/lfd/, rust/loopflow/src/engine/, python/loopflow/, rust/loopflow/src/engine/builtins/steps/, rust/loopflow/src/engine/builtins/flows/

### Runtime
- **lfd**: not queryable — `lfq show model` fails (lfd not running or wave not registered under new name)

### Progress

**Shipped this week (landed to main):**
- #593: worker capacity, xor/loop engine, ops→op rename (merged 2026-03-21)
- #584: rename tend→garden, add VSM governance flows, xor/or/loop engine (merged 2026-03-18)
- #569: algedonic signals — repair lineage, error classification, and escalation (merged 2026-03-18)

14 commits touched model-area paths on main in the past week. Heavy activity across engine, lfd, and builtins.

**In flight:**
- Worktree at `/Users/jack/src/loopflow.chord-model` on branch `jack-heart.chord-model.20260320_2226` — clean working tree, 0 commits ahead of main (fresh branch).
- No worktree exists yet under the new `model` name.

### Items

| # | Title | Status |
|---|-------|--------|
| 1-wave-modes | Wave Modes (`flow` replacing `manual`) | queued — not started |
| 2-concurrent-ingest | Concurrent Ingest | queued — PM providers ready, atomic arbitration not built |
| 2-planning-flow | Planning Flow | queued — VSM governance flows shipped, planning tree traversal remains |
| 2-wave-crons | Wave Crons | queued — `crons` config field not implemented |
| 3-tend-flow-steps | Tend Flow Steps | queued — structural wiring exists, live validation against lfd needed |
| 3-vsm-flow | VSM Flow | queued — four builtin steps exist, single-pass `lf vsm` not wired |
| 3-wave-discovery-and-root-chord | Wave Discovery and Root Chord | queued |
| 4-api-expansion | API Expansion | queued (speculative) |
| 4-dag-and-nested-chords | DAG and Nested Chords | queued (speculative) |
| 4-letta-integration | Letta Integration | queued (speculative) — blocked on live garden output |
| 4-wave-mutation | Wave Mutation | queued (speculative) |

### Blocks
- No CI failures on model branches (most recent PR #593 merged clean).
- No open questions in scratch/.
- lfd not running prevents live validation of tend-flow-steps.

### Open PRs
None.

### Unlanded Branches
| Branch | Commits ahead | Diff stats | PR |
|--------|---------------|------------|----|
| origin/jack-heart.chord-model.20260320_2137 | stale | large | none (pre-merge state of #593) |
| origin/jack-heart.chord-model.20260319_1840 | 5 | large | #593 merged from here |
| origin/jack-heart.chord-model.20260318_1226 | 5 | large | #584 merged |
| origin/jack-heart.chord-model.20260318_0010 | landed | — | #581 merged |
| origin/jack-heart.chord-model.20260317_2324 | stale | — | none |
| origin/jack-heart.chord-model.20260318_0020 | stale | — | none |
| origin/jack.chord-model.20260316_1856 | stale | — | none |

All branches are remnants of landed PRs or abandoned iterations. No significant unlanded work.

---

## Wave: macos

### Config
- **Flow**: build-or-silent
- **Mode**: manual
- **Direction**: care, clarity
- **Area**: swift/Concerto/, swift/LoopflowCore/

### Runtime
- **lfd**: not queryable — `lfq show macos` fails (lfd not running or wave not registered under new name)

### Progress

**Shipped this week (landed to main):**
- #594: interactive checkpoints, wave workspaces, pm bootstrap, and garden rename (merged 2026-03-21)
- #592: wave workspaces with terminal-native keyboard routing (merged 2026-03-20)
- #588: terminal multiplexer, attention lifecycle, flow engine extensions (merged 2026-03-20)
- #587: terminal sessions, attention lifecycle, wave workspace UI (merged 2026-03-19)

4 commits touched swift/ paths on main in the past week.

**In flight:**
- Worktree at `/Users/jack/src/loopflow.agent-embedding` on branch `jack-heart.agent-embedding.20260320_2148`.
- 5 commits ahead of main: kickoff design doc for 04-window-composition, gate pass, compress, roadmap updates.
- Scratch contains `04-window-composition.md` (validation checklist) and gate review artifacts.
- No worktree exists yet under the new `macos` name.

### Items

| # | Title | Status |
|---|-------|--------|
| 04-window-composition | Window Composition — Polish | in-flight — design doc written, gate passed, on active branch |
| 4-wave-lifecycle-ui | Wave Lifecycle UI | queued — worktree management and step-level run history gaps |
| 4-portfolio-view | Portfolio View | queued (speculative) — multi-repo, multi-wave conductor dashboard |
| 4-calibration-view | Calibration View | queued (speculative) — garden flow's human checkpoint UX |
| 4-concerto-release-ui | Concerto Release UI | queued (speculative) — show release targets, trigger from app |

### Blocks
- No CI failures on macos branches.
- No open questions in scratch/.

### Open PRs
None.

### Unlanded Branches
| Branch | Commits ahead | Diff stats | PR |
|--------|---------------|------------|----|
| origin/jack-heart.agent-embedding.20260320_2148 | 5 | 256 files, +19512/-4852 | none (active worktree — includes pre-merge state + new work) |
| origin/jack-heart.agent-embedding.20260320_1445 | landed | — | #594 merged |
| origin/jack-heart.agent-embedding.20260318_1627 | landed | — | #592, #588 merged |
| origin/jack-heart.agent-embedding.20260318_1241 | landed | — | #587 merged |
| origin/jack-heart.agent-embedding.20260317_1347 | stale | — | none |
| origin/jack.agent-embedding.20260316_1856 | stale | — | none |

Active work on `20260320_2148` — window composition design doc and gate artifacts. All other branches are landed PR remnants.

---

## Wave: lfd

### Config
- **Flow**: build-or-silent
- **Mode**: manual
- **Direction**: clarity, simplicity
- **Area**: rust/loopflow/src/lfd/, rust/loopflow/src/lf/, python/loopflow/, wave/lfd/

### Runtime
- **lfd**: not queryable — `lfq show lfd` fails. lfd itself appears to not be running.

### Progress

**Shipped this week (landed to main):**
- #591: observe wave CLI runs through runtime journals (merged 2026-03-21)
- #578: algedonic signals with repair backoff (merged 2026-03-18)

15 commits touched lfd-area paths on main in the past week (many shared with model wave area).

**In flight:**
- Worktree at `/Users/jack/src/loopflow.lfd` on branch `jack-heart.lfd.20260320_2312`.
- 5 commits ahead of main: land artifacts, PR prep, wave README update, post-ship item updates.
- **PR #596 open**: "lfd: shared flow execution engine with runtime journals" — **rust-test CI failing**.

### Items

| # | Title | Status |
|---|-------|--------|
| 1-real-cli-executor | Real CLI Executor | in-flight — journal contract shipped (#591), PR #596 open with CI failure |
| 1-daemon-hosted-shells | Daemon-Hosted Shells | queued — PTY/session manager design complete, depends on executor convergence |

### Blocks
- **PR #596 has failing CI** — `rust-test` job fails. `tests-result` fails as a consequence. All other checks (python, e2e, docker, swift, concerto-ui, scratch-clear) pass.
- lfd process not running locally prevents runtime validation.

### Open PRs
| # | Title | CI | Age |
|---|-------|----|-----|
| 596 | lfd: shared flow execution engine with runtime journals | rust-test FAILED | ~12h |

### Unlanded Branches
| Branch | Commits ahead | Diff stats | PR |
|--------|---------------|------------|----|
| origin/jack-heart.lfd.20260320_1750 | 5+ | large | #596 open (CI failing) |
| origin/jack-heart.lfd.20260320_2312 | 5 | 309 files, +21310/-7299 | none (active worktree, newer than PR branch) |
| origin/jack-heart.lfd.20260319_1333 | landed | — | #591 merged |

The active worktree (`20260320_2312`) is ahead of the PR branch (`20260320_1750`). PR #596 was created from the older branch.

---

## Wave: pm

### Config
- **Flow**: build-or-silent
- **Mode**: manual
- **Direction**: clarity, simplicity
- **Area**: rust/loopflow/src/ops/, wave/pm/ (plus Rust lf/lfd/engine modules in yaml)

### Runtime
- **lfd**: not queryable — `lfq show pm` fails.

### Progress

**Shipped this week (landed to main):**
- #595: Notion provider with OAuth, page blocks, and bootstrap (merged 2026-03-21)
- #590: priority buckets, notion prep, and init rework (merged 2026-03-20)
- #589: add export, push-diff, and per-wave bootstrap (merged 2026-03-21)
- #586: bootstrap CLI with init, pull, and status commands (merged 2026-03-19)
- #572: add Linear support and lf ops pm sync (merged 2026-03-18)
- #564: Asana OAuth auth, client, and wave export (merged 2026-03-18)
- #562: PM integration foundation (merged 2026-03-18)

7 commits touched pm-area paths on main. Highest PR volume of any wave this week.

**In flight:**
- Worktree at `/Users/jack/src/loopflow.pm` on branch `jack-heart.pm.20260320_2239`.
- 5 commits ahead of main: land artifacts, Notion client fix after rebase, scratch cleanup, roadmap consolidation.

### Items

| # | Title | Status |
|---|-------|--------|
| 1-ingest-auto-import | Ingest Auto-Import | queued — `pm_pull` exists, ingest integration not wired |
| 2-asana-rich-text | Asana Rich Text | queued — markdown↔HTML converter needed |
| 2-provider-auth | Provider Auth (typed auth + OAuth-only PM) | queued — combined from previous typed-auth + oauth-only items |
| 3-notion-readme-sync | Notion README Sync | queued — converters exist, wave-level pointer needed |
| 4-item-lifecycle-comments | Item Lifecycle Comments and Completion | queued (speculative) |
| 4-pm-sync-steps-and-flow | PM Sync Steps and Flow | queued (speculative) |

### Blocks
- No CI failures on pm branches (PR #589 merged clean after earlier scratch-clear issue was resolved).
- No open questions in scratch/.

### Open PRs
None.

### Unlanded Branches
| Branch | Commits ahead | Diff stats | PR |
|--------|---------------|------------|----|
| origin/jack-heart.pm-notion.20260320_1503 | landed | — | #595 merged |
| origin/jack-heart.pm-notion.20260319_1248 | landed | — | #590 merged |
| origin/jack-heart.pm.20260319_1257 | landed | — | #589 merged |
| origin/jack-heart.pm.20260318_1553 | landed | — | #586 merged |
| origin/jack-heart.pm.20260318_0032 | stale | — | none |
| origin/jack-heart.pm.20260317_2323 | stale | — | none |
| origin/jack-heart.pm.20260317_1653 | stale | — | none |
| origin/jack-heart.pm.20260317_1150 | stale | — | none |

All branches are landed PR remnants. Active worktree (`20260320_2239`) has post-merge cleanup commits only.

---

## Wave: ios

### Config
- **Flow**: build-or-silent
- **Mode**: manual
- **Direction**: care, reliability
- **Area**: rust/loopflow/src/lfd/, python/loopflow/, swift/, docker/

### Runtime
- **lfd**: not queryable — `lfq show ios` fails.

### Progress

**Shipped this week (landed to main):**
No ios-specific PRs landed. Swift-area commits from macos wave (#594, #592, #588, #587) touch shared paths.

**In flight:**
- Worktree at `/Users/jack/src/loopflow.dogfood` (old name) on branch `jack-heart.dogfood.20260318_0032` — appears stale (same commit as many other stale worktrees: `123e32b7b`).
- No worktree exists under the new `ios` name.
- No active branches.

### Items

| # | Title | Status |
|---|-------|--------|
| 1-concerto-local | Concerto Local | queued — minimal item, no body |
| 1-ios-testflight-distribution | iOS TestFlight Distribution | queued — needs App Store Connect setup, CI path |
| 1-mac-mini-server | Mac Mini Server | queued — launchd, remote parity, monitoring |
| 1-phone-deploy | Phone Deploy | queued — remote connection from iPhone, block queue on phone |
| 3-team-workflow | Team Workflow | queued — multi-user auth, shared waves |

### Blocks
- No active work. Wave has not started since restructuring from `dogfood`.
- Depends on lfd being operational for remote validation items.
- iOS CI path (TestFlight) not yet built.

### Open PRs
None.

### Unlanded Branches
| Branch | Commits ahead | Diff stats | PR |
|--------|---------------|------------|----|
| origin/jack-heart.dogfood.20260318_0032 | stale | — | none |
| origin/jack-heart.dogfood.20260317_1840 | stale | — | none |

All branches stale. No ios-named branches exist.

---

## Cross-Wave

### Shared area overlap
- **model ↔ lfd**: Both waves list `rust/loopflow/src/lfd/` in their area. PR #596 (lfd) modifies engine code that model also owns. The lfd wave's real-cli-executor work restructures how flows execute, which directly affects model's tend-flow-steps and vsm-flow items.
- **macos ↔ ios**: Both include `swift/` paths. Macos owns `swift/Concerto/` and `swift/LoopflowCore/`; ios owns all of `swift/`. iOS TestFlight distribution depends on the same Xcode project macos is actively modifying.
- **lfd ↔ ios**: iOS remote validation items (mac-mini-server, phone-deploy) depend on lfd being operational. lfd's daemon-hosted-shells item directly enables the remote connection ios needs.
- **pm ↔ model**: Concurrent ingest (model) depends on PM providers for atomic arbitration. PM's ingest-auto-import wires the same integration from the PM side. These are two halves of the same feature.

### Dependency ordering
- **model gates macos**: Portfolio view and calibration view in macos need wave state data that model's tend-flow-steps and wave-discovery items would formalize. Per chord review, these aren't formally blocked — they can read raw state — but the data model would make them cleaner.
- **lfd gates ios**: Every ios item except concerto-local depends on lfd running remotely.
- **lfd gates model**: Tend-flow-steps requires live lfd to validate. VSM flow needs live runtime.

### Recent cross-wave activity
- Macos PRs (#587, #588, #592, #594) included engine/flow changes in model's area — expected, as Concerto features require new steps and flow extensions.
- PR #596 (lfd) touches shared infrastructure across model and lfd areas.

---

## Raw Signals

- **lfd is down**: All five `lfq show` calls fail. Either lfd is not running or auth tokens expired. No runtime state available for any wave.
- **Wave rename incomplete**: Worktrees still use old names (chord-model, agent-embedding, dogfood). No worktrees exist under new names (model, macos, ios). Waves are not registered in lfd under any name.
- **Branch proliferation**: 28 remote branches across member waves, nearly all stale remnants of landed PRs. 35 worktrees on disk including run artifacts. No cleanup has run recently.
- **Unstaged deletions on root branch**: This branch (redesign) has unstaged changes deleting scan/scan-plan, scan/scan-report, and several flow YAML files from builtins — leftover from the restructuring that hasn't been committed yet.
- **PR #596 is the only open PR** and it has a rust-test CI failure. The lfd worktree has moved ahead of the PR branch (newer branch `20260320_2312` vs PR branch `20260320_1750`), suggesting the failure may already be addressed locally but not pushed.
- **Velocity**: 17 PRs landed to main in the past week. PM had the highest volume (7 PRs). Macos had 4, model had 3, lfd had 2, redesign had 1.
- **ios is dormant**: No PRs, no active branches, stale worktree under old name. The wave was just restructured from dogfood and hasn't started work under its new identity.
- **pm is post-foundation**: Seven PRs shipped the full PM stack (Asana, Linear, Notion). Remaining items are integration and polish, not foundation.
