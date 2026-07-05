---
requires: wave-agent-design.md (the design this code implements)
produces: the guided read of the diff — tour order, line-by-line spots, divergences, findings
---
# Reading the wave-agent diff

24.8k insertions / 3k deletions, 164 files, 53 commits vs `origin/main`. Four
layers, stacked; each section below is one layer with the question to hold
while reading, the map, and the line-by-line spots. §5 is where the code
diverges from wave-agent-design.md; §6 is the ranked findings list.

Rough sizes (non-test code is much smaller than the raw line counts —
journal.rs is 1494 lines but ~230 non-test):

| Layer | Where | Lines | The question to hold |
|---|---|---|---|
| 1. Wave server core | `rust/loopflow/src/lfd/wave/` | ~7.5k | Is the journal really the only truth? |
| 2. Harness | `rust/loopflow/src/lfd/conversations/` | ~5.6k | Where does vendor drift bite? |
| 3. lf surface | `lf/session.rs`, `commands/{chat,q,memory}.rs`, `engine/wave_context.rs` | ~2.6k | Do the doors degrade correctly? |
| 4. Concerto | `swift/` WaveChat stack | ~2.4k | Does the viewer ever participate? |

Suggested order: 1 → 3 → 2 → 4. The core defines the vocabulary; the lf
surface is its client and the second-most decision-dense; the harness is big
but mechanical once you've seen the event types; Concerto is a mirror of
wire shapes you'll already know.

---

## 1. The wave server core (`lfd/wave/`)

The claim under test: *every projection is a fold over one append-only
journal; the mind is a 4-state machine; one brain per wave.*

### Map
| File | One line | Anchor |
|---|---|---|
| `journal.rs` | Event types, the single writer, the pure folds, console narrator | `EventKind` journal.rs:168, `Journal::append` :628, `fold_thread` :677 |
| `runtime.rs` | All in-process state as a fold behind one mutex; append+broadcast choke point | `WaveRuntime::open` runtime.rs:137, `TurnSink::on_delta` :686 |
| `mind.rs` | The persistent vendor thread + event-driven scheduler (message/steer/interrupt/heartbeat/failure-cap/revive) | `run_mind` mind.rs:419, the `on_*` handlers :551–:819 |
| `state.rs` | MindState (Idle/Turning/Interrupting/Failed) + `can_transition` table | state.rs:59 |
| `registry.rs` | Store-direct one-brain registration + polling worker observer | `register` registry.rs:176, `StoreObserver` :321 |
| `server.rs` | Thin HTTP: /health, /conversation(+SSE), /messages, /memory; endpoint file ops | `router` server.rs:139, `messages_handler` :169 |
| `mod.rs` | `lf wave` entry: worktree bootstrap → registry → runtime → mind → axum | `serve` mod.rs:155 |
| `memory.rs` | MEMORY.md handle (reads free; writes only via runtime) | 85 lines, trivial |
| `supervisor.rs` | Detached-task registry — **mostly dead, see §6.4** | supervisor.rs:32 |

### Where the design claims live
- **Append choke point**: `Journal::append` journal.rs:628 — only writer, flush + narrate per event. All producers go through the runtime mutex.
- **Pure folds**: `fold_thread` journal.rs:677 (thread + state + thread_id + pending), `fold_workers` :793, read-only `read_events` :253.
- **Answers-based queue**: queue = UserMessages not named in any `TurnStarted.answers` / `TurnSteered.answers` — `mark_consumed` journal.rs:775, minted at `TurnSink::expect_answers` runtime.rs:674.
- **can_transition**: state.rs:59. Non-obvious bits: same-turn-id guard on Turning→Interrupting :64, Failed is sticky :67.
- **Janitors** (three): boot (open turns → Failed, state → Idle) runtime.rs:146; torn-tail truncation `Journal::open` journal.rs:601; interrupt-deadline `on_interrupt_deadline` mind.rs:647 → `force_finalize_open_turn` runtime.rs:447.
- **One-brain** (three layers): endpoint-file floor mod.rs:169; store row + pid probe `live_brain_after_probe` registry.rs:247 / `process_alive` :290; `ensure_wave_row` born `paused=true` registry.rs:157 (the legacy-ticker interaction).

### Read line-by-line
1. `runtime.rs:137-190` (`open` + boot janitor) and :447-628 (`force_finalize` + the three `sink_*`) — the store-is-truth claim is enforced or broken here.
2. `mind.rs:453-819` — the whole scheduler: biased select, steer degrade, interrupt deadline, failure cap (`MAX_CONSECUTIVE_TURN_FAILURES=3`), revive. The subtle ordering lives here.
3. `journal.rs:557-616` (`open`: tail truncation, version gate) + :677-831 (the folds; `mark_consumed` is the queue's correctness core).
4. `registry.rs:176-296` — one-brain enforcement, force-takeover, crashed-row reconciliation.
5. `state.rs:59-71` — tiny; read once, everything keys off it.

Skim: memory.rs, supervisor.rs, server.rs handlers (but do read `messages_handler` :169 for the op/from validation matrix and `remove_endpoint` :372 for the own-address guard).

### Data-flow chains (for reference while reading)
- **Boot**: run mod.rs:66 → wave_worktree :107 → resolve_registry :119 → serve :155 → live_endpoint check → `WaveRuntime::open` → register → spawn StoreObserver → spawn run_mind → write_endpoint → axum.
- **Message**: POST /messages → deliver_user_message runtime.rs:476 → journal append + broadcast + inbox → mind `on_message` :564 → `start_queued_turn` :734 → `send_turn` :760.
- **Turn streaming**: harness events → `EventAdapter::feed` mind.rs:283 → `TurnDelta` → `TurnSink::on_delta` runtime.rs:686 → journal + open_turn + SSE broadcast.
- **Interrupt**: `on_interrupt` mind.rs:610 → `begin_interrupt` (Turning→Interrupting) → harness.interrupt() → cooperative finish OR deadline janitor :647.
- **Worker obs**: StoreObserver 10s poll + `refresh_observations` before each turn → `journal_worker_dispatched/finished` → `<in_flight>` in heartbeat seed.
- **Shutdown**: ctrl_c → mind.abort, observer.abort, deregister, remove_endpoint. (SIGTERM/HUP: hooks only — see §6.2.)

---

## 2. The harness (`lfd/conversations/`)

The claim under test: *the Harness trait is the 1b/1c boundary; codex
app-server is the live path; claude/opencode are conformance-tested seams.*

### Map
| File | One line | Anchor |
|---|---|---|
| `types.rs` | Wire vocabulary: one `Lifecycle` enum, `ConversationItem` union, `ConversationEvent` channel payload | Lifecycle types.rs:23, ConversationEvent :116 |
| `turns.rs` | `ChatTurn` + `TurnDelta` (the increments the journal hangs off) | ChatTurn turns.rs:34, TurnDelta :80 |
| `harness/mod.rs` | The trait, capabilities, factory | Harness trait mod.rs:64 |
| `harness/codex.rs` | app-server JSON-RPC driver: spawn, handshake, reader/writer tasks, steer/interrupt | start_inner codex.rs:539, process_notification :131 |
| `harness/codex_mapping.rs` | codex items → ConversationItems | build_item :93 |
| `harness/claude.rs` + `claude_mapping.rs` | per-turn `claude -p` subprocess; stream-json NDJSON mapping (synthesizes Edit diffs) | process_line claude_mapping.rs:105 |
| `harness/opencode.rs` + `opencode_mapping.rs` | HTTP+SSE driver; manufactures turn boundaries from `session.status` | map_event opencode_mapping.rs:66 |
| `harness/lf_tag.rs` | streaming `<lf:suggest_actions>` extractor (buffers split tags) | consume_text lf_tag.rs:15 |
| `harness/conformance_tests.rs` | trace replay through the *production* dispatch fns | replay_* :34/:77/:114 |
| `opencode_runtime.rs` | orphaned `opencode serve` reaper (pid registry JSON) | reap :32 |

### The capability matrix (pinned by test mod.rs:173)
| | steer | interrupt | resume handle |
|---|---|---|---|
| codex | ✓ `turn/steer` RPC + expectedTurnId codex.rs:463 | ✓ `turn/interrupt` (cooperative) :486 | thread id announced at start; no set-override |
| claude | ✗ → TurnAlreadyInProgress, caller queues | SIGKILL the per-turn process, **no grace** claude.rs:250 | `--resume <session_id>` captured from first turn |
| opencode | ✗ → server-side queue | POST /abort; interrupted status is a *heuristic* stamp opencode.rs:178 | id from POST /session |

### Read line-by-line
1. `codex.rs:539-782` (`start_inner`) — spawn, process-group + interrupt-hook wiring, writer/reader tasks, handshake, thread/start. Every fragility note traces here.
2. `codex.rs:131-251` (`process_notification`) — the shared production+replay dispatch; the codex turn state machine; the belt-and-suspenders auto-accept :692.
3. `lf_tag.rs:14-142` — byte-offset buffer logic for tags split across chunks; line-start gating so inline examples pass through.
4. `opencode_mapping.rs:66-139` — the SessionState machine that manufactures turn boundaries opencode doesn't provide.
5. `claude_mapping.rs:105-247` — the NDJSON router and the `result`→turn-completion return contract.

Skim: types/turns (read the enums), both `build_item` helpers, opencode_runtime.rs, common.rs, all inline test blocks.

### What conformance tests do NOT cover
No spawn/handshake/framing/SSE; **no steer or interrupt test at driver
level** (capability flags only); no claude interrupted-turn trace. And the
traces are **hand-authored, not machine-recorded** (manifest says
`opencode_version: "unknown"`) — they pin *our interpretation* of the
protocol, not the vendor's actual output. Highest-risk seam in the layer.

---

## 3. The lf surface (exec doors + ambient context)

The claim under test: *one calling convention; every actor speaks the same
verbs; degrade is silent-and-correct outside a wave.*

### Map
| File | One line | Anchor |
|---|---|---|
| `lf/session.rs` | self-registration: bare `lf <flow>` inside a wave becomes a visible child session, daemonless | classify_run_context session.rs:68, register_run :92 |
| `lf/commands/chat.rs` | `lf chat`: resolve target wave (env → worktree → --wave/--parent), POST say-op to live server | resolve_target chat.rs:131 |
| `lf/commands/memory.rs` | `lf memory show|update|add` — server holds the pen; reads fall back to direct file | run memory.rs:19 |
| `lf/commands/q.rs` | `lf q worker run`: daemonless dispatch — placement, capacity gate, run+session rows, tmux | dispatch q.rs:100 |
| `engine/wave_context.rs` | ambient `<lf:wave-chat-recent>` + `<lf:wave-memory>` in every flow run; live → journal → empty | gather_wave_chat wave_context.rs:121 |
| `bin/lf.rs` + `lf/mod.rs` | registration wired before dispatch; `lf goal`/`lf loop` deleted; `wave` = the server (alias "loop") | run_label + registration in main |
| `lfd/triggers/loop_ticker.rs` | one-brain gate: live WaveAgent session ⇒ ticker skips the wave | tick check :60-79 |
| `lfd/store/migrations.rs` | 048 + 049 rename-convergence healing (`wave_run_id` → `run_id`, `wave_runs` → `runs`), tolerated-error idempotence | :205-209 |

### The env contract (the crux — memorize this)
`LFD_SESSION_ID` **without** `LFD_SESSION_INHERITED` = "this process owns
the row" (dispatcher created it) → **adopt**, don't register. **With** the
marker = ancestor's row → register a child under it. `register_run`
overwrites `LFD_SESSION_ID` to the new row for descendants (session.rs:143),
so grandchildren chain correctly. `lf q worker run` deliberately omits the
inherited marker (q.rs:163-173) so the worker adopts its pre-created row.

### Read line-by-line
1. `session.rs:64-189` — classify/register/adopt. Everything keys off this.
2. `chat.rs:131-241` (`resolve_target`) — the resolution decision tree (env → worktree → store row env → `.wave-endpoint` file), shared by memory.
3. `q.rs:100-203` (`dispatch`) — parent/capacity validation + the exact env handoff.
4. `wave_context.rs:121-205` — live→journal→empty fallback and the deliberate raw-socket blocking GET (:164; reqwest::blocking panics inside a runtime).
5. `bin/lf.rs` main diff — where registration is invoked and completed.

Skim: prompt.rs diff (mechanical section rendering), helpers.rs placement/worktree (git mechanics), migrations (read the two SQL headers + `is_tolerated_migration_error`).

### Degrade behavior (verify these while reading)
- Ambient context outside a wave / dead server: silently empty (live 1s timeout → read-only journal fold → None). Never blocks a flow.
- `lf chat` / `lf memory write` with no live server: **hard error** naming `lf wave <name>` ("queuing not implemented yet") — *not* the silent exit-0 drop the design describes; see §5.6.
- `lf memory show`: works offline (direct file read; only writes need the pen).

---

## 4. Concerto (viewer)

The claim under test: *attaches via substrate only; whole-turn snapshots +
id-replace make a lossy stream self-healing; composer verbs key off mind
state.*

### Map
| File | One line | Anchor |
|---|---|---|
| `WaveChatClient.swift` | the whole client: endpoint poll loop, hand-rolled SSE parser, id-replace upsert, composer verb table, POST | SSEFrameParser :49, stream :258, upsert :314, composerVerbs :132 |
| `WaveOrigin.swift` | worktree → origin resolution (mirrors Rust `wave_origin`), memoized; the single choke point for all paths | resolve :20 |
| `ChatTurn.swift` | wire DTOs, manual codec, DTO discipline | ConversationItem :22, ChatTurn :126 |
| `AgentSession.swift` | JSONValue (now Codable, throwing decode); old parallel item model **deleted** at HEAD | JSONValue :4 |
| `WaveChatView.swift` | pane: transcript, Start-wave flow, composer dispatch + failed-send restore | startWave :193, perform :304 |
| `MessageRow.swift` | speech prominent, items as collapsible cards, `from` byline | :36, ConversationItemCard :114 |
| `MacLocalWaveAgentLauncher.swift` | launch `lf wave` in detached tmux; `lf help wave` capability probe; double-launch guard | resolveWaveCapableLf :130 |

Note: the five "dirty" files from session start are now committed at HEAD
(`669076042 "compress: collapse Swift chat item model"`) — that commit is
DTO consolidation (one item model, `tool.input` as structured `JSONValue`,
−145 lines), not new rendering work. The from-byline/activity-collapse
landed earlier.

### Read line-by-line
1. `WaveChatClient.swift:49-88` + :258-324 — the SSE parser (the AsyncBytes.lines bug fix — the load-bearing "why" of this layer, pinned by a verbatim captured frame in tests), clear-state-on-open, id-replace merge.
2. `WaveChatClient.swift:99-143` + `WaveChatView.swift:304-329` — composer verb table end-to-end + failed-send restore.
3. `MacLocalWaveAgentLauncher.swift:96-160` — candidate order + why the probe is `lf help wave` and not `lf wave --help`.
4. `ChatTurn.swift:48-123` — the manual codec; cross-read ContractTests.swift:98-164.
5. `WaveOrigin.swift:35-53` — the git logic everything's paths hinge on.

Skim: WaveChatRendering.swift, MessageRow view-builders, launcher tests (read one, the rest rhyme).

DTO discipline: **clean.** No `??` field fallbacks in decode paths;
unknown item types decode to `.unknown` (forward-compat, not a default);
`JSONValue` decode now throws instead of swallowing to `.null`; `from`
omission → nil, asserted in tests.

---

## 5. Where the code diverges from the design doc

Each of these is "wave-agent-design.md / shakedown says X; the code does Y."
None invalidates the architecture; all are worth a deliberate accept-or-fix.

1. **Interrupt is not "cooperative cancel → grace → kill" anywhere.**
   Codex is cooperative (`turn/interrupt`) with a *deadline janitor* but no
   grace window in `stop()` — straight to group-SIGKILL (codex.rs:510-516).
   Claude interrupt is an immediate SIGKILL of the per-turn process, no
   grace (claude.rs:250). The design's three-stage story is aspirational.
2. **Steer mechanism**: design says "app-server pending_input injection";
   code uses the `turn/steer` RPC with `expectedTurnId` (codex.rs:463).
   Better than designed (optimistic concurrency built in) — update the doc.
3. **`/health.subagents` ≠ workers in flight.** Health reports the
   Supervisor's tokio-task count, which is always 0 because dispatch went
   daemonless (`lf q worker run`). Real in-flight workers only appear in the
   heartbeat `<in_flight>` fold. (See §6.4 — Supervisor is mostly dead.)
4. **SIGTERM/SIGHUP ≠ Ctrl-C.** Graceful shutdown future is `ctrl_c()` only
   (mod.rs:302). On TERM/HUP the interrupt hooks deregister + remove the
   endpoint file, but `mind.abort()` / `observer.abort()` /
   `supervisor.shutdown_all()` never run — the process just exits (the mind's
   codex child dies via process-group/kill_on_drop, so no orphans, but the
   path is asymmetric and the README overstates it).
5. **Migration healing is split 048/049**, not "migration 049" as the
   shakedown notes say: 048 heals `terminal_sessions.wave_run_id`; 049 does
   `wave_runs`→`runs` + `agents`/`fork_runs`.
6. **`lf chat` outside a live server is a hard error, not a silent drop.**
   The design's pubsub semantics ("publishing to no subscriber drops; exits
   0") holds for *no wave context* (ambient no-op), but a resolved wave with
   no live server errors with "queuing not implemented yet" (chat.rs:118).
   Deliberate MVP floor — but it contradicts the "correct pubsub semantics"
   paragraph, and worker reports racing a server restart will fail loudly.
7. **Section tags** are `<lf:wave-chat-recent>`/`<lf:wave-memory>`, not the
   design's `<wave_chat_recent>`/`<wave_memory>`. Cosmetic.

## 6. Findings (ranked; each verified by a reader against the source)

**Fix-worthy (all six FIXED on this branch, 2026-07-04 session):**
1. **Steer race orphans a pending message** (mind.rs:588): if
   `harness.send_input` succeeds but the turn closes before
   `journal_steered` lands, the message reached the vendor but gets no
   `answers` marker — it stays pending in the fold forever and is re-sent
   as a queued turn on every restart. FIXED: `journal_steered` now consumes
   against the just-closed turn at the boundary (test:
   `journal_steered_consumes_against_live_or_just_closed_turn`); the
   no-turn-at-all path re-queues instead of orphaning.
2. **Self-registered rows can clobber a reconciler's terminal write**
   (session.rs:249-254). FIXED: both paths now re-read before the terminal
   write; the `adopted` flag is deleted (one path, one guard).
3. **`is_terminal_harness_error` had a dead branch** (harness/mod.rs:35):
   `claude_harness_crashed` is never emitted — and can't be: claude runs one
   subprocess per turn, so a crash fails the turn, never the session. FIXED:
   branch deleted, reasoning documented at the fn.
4. **Supervisor was vestigial** (supervisor.rs). FIXED: deleted whole
   (dispatch is daemonless; workers are their own tmux sessions);
   `/health.subagents` (always 0) → `/health.workers` =
   `in_flight_workers().len()`.
5. **Three hand-synced copies of the turn-growth join**. FIXED: extracted to
   `ChatTurn::absorb_item`/`push_text` (turns.rs); runtime sink, journal
   fold, and EventAdapter all call the one function.
6. **`run_label` mislabeled bare `lf`** as `"chat"`. FIXED: registers as
   `"interactive"`.

**Accepted-risk (know they're there):**
- Codex driver hardcodes 0.142.5 wire strings; unknown methods silently
  no-op (codex.rs:247). No live/CI conformance guard.
- Conformance traces are hand-authored, not recorded; no steer/interrupt
  driver-level tests.
- OpenCode interrupted-status is a timing heuristic (opencode.rs:178);
  its `send_request_with_retry` re-POSTs a possibly-processed message on
  5xx (opencode.rs:490).
- Mutex-lock `.expect("poisoned")` throughout reader tasks (idiomatic, but
  a poisoned lock kills a turn silently); `serde_json::to_string(turn)
  .unwrap_or_default()` server.rs:312 would emit a blank SSE frame.
- `ChatTurn.sequence` sentinel: non-`turn-<int>` ids collapse to `.max`
  and order by id string (ChatTurn.swift:154) — silent if the id scheme
  ever changes.
- Migration tolerance matches on sqlite/postgres error-string substrings.
- `is_tolerated_migration_error` + born-`paused=true` wave rows interact
  with legacy tickers — covered by tests, but it's the seam between old
  and new worlds.

## 7. After the read

The natural next moves, once you've formed your own view:
- File the §6 fix-worthies (1, 2, 5 are real correctness; 3, 4, 6 are
  hygiene) — either this branch or the Asana roadmap per the PR-scope call.
- Reconcile the design docs with §5 (especially the interrupt story and
  the chat-degrade semantics — those are written down as decided behavior).
- The dead-branch/vestigial items (§6.3, §6.4) are exactly the "would
  deleting code make the system more true?" review-ritual question.
