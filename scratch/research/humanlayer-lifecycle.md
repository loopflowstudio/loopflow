# HumanLayer session lifecycle & durability

Sources: `hld/session/{types,manager}.go`, `hld/store/sqlite.go`; DeepWiki;
12-factor factors 5/6/12. Grounded vs our `rust/loopflow/src/lfd/types/{session,wave}.rs`,
`executor/wave/mod.rs`.

## 1. Session state machine
| status | meaning | durable | resumable |
|---|---|---|---|
| `draft` | pre-execution config; not launched | yes | n/a |
| `starting` | draft → active, transient | yes | — |
| `running` | Claude Code executing | yes | — |
| `waiting_input` | blocked on a tool approval | yes | yes (on decision) |
| `interrupting` | received interrupt, shutting down | yes | — |
| `interrupted` | interrupted, **can be resumed** | yes | **yes** |
| `completed` | finished; **can be forked** | yes | **yes (fork)** |
| `failed` | errored; cannot continue | yes | no |
| `discarded` | draft thrown away | yes | no |

Transitions (no guard table in code — a gap; issue #954 "stuck in Interrupting"):
`draft→starting→running` (`LaunchDraftSession`); `running→completed|failed` on
exit; `running→waiting_input` on approval need, back on decide;
`running→interrupting→interrupted` (`InterruptSession`); `interrupted→running` and
`completed→running` (`ContinueSession`, as a new child, §3); `draft→discarded`.
**Every status lives in the `sessions` SQLite row** → survives restart. The
distinction that matters: `interrupted` (resume) and `completed` (fork) are
re-launchable; `failed`/`discarded` terminal.

## 2. Identity model — four ids, three layers
- **`id`** (TEXT PK, UUID) — the daemon's session handle; what API/UI address.
- **`run_id`** (TEXT NOT NULL UNIQUE) — **correlation key for approvals**; matches
  a blocked tool call back to its session across the process boundary; the
  injected MCP server carries it, `approvals` keys on it.
- **`claude_session_id`** (nullable) — Claude Code's *own* conversation id,
  captured *after* launch; the handle passed to `--resume`/`--fork`; the daemon
  doesn't own it.
- **`parent_session_id`** (nullable self-FK) — lineage for forks/continuations.

No separate `thread_id` — the thread is `claude_session_id` + the walk up the
`parent_session_id` chain. The layering exists because the daemon needs a stable
id *before* Claude exists (`id`,`run_id`), a correlation id that crosses the MCP/
HTTP boundary durably (`run_id`), and a *borrowed* id it doesn't control
(`claude_session_id`). **Maps ~1:1 onto our `Session{id,wave_id,run_id,
parent_session_id}` — we have NO `claude_session_id` analogue and no durable
thread handle. That's the gap.**

## 3. Resume & continue — forks, never mutates
`ContinueSession` **always creates a new session row**:
```go
sessionID := uuid.New(); runID := uuid.New()
dbSession := NewSessionFromConfig(sessionID, runID, config)
dbSession.ParentSessionID = req.ParentSessionID
config := claudecode.SessionConfig{
    Query: req.Query, SessionID: parentSession.ClaudeSessionID, ForkSession: true }
```
New `id`, new `run_id`, `parent_session_id` = source. Context **not copied by the
daemon** — it passes the parent's `claude_session_id`, Claude reloads its own
conversation; `ForkSession:true` branches (new `claude_session_id`).
Reconstruction (`GetSessionConversation`) walks ancestry, folds
`conversation_events` chronologically. Net: **continuation = append-only lineage
of immutable sessions; forks branch.**

## 4. Durable pending requests (load-bearing)
1. Claude calls injected MCP `request_permission` → MCP `POST /api/v1/approvals`
   → daemon writes an `approvals` row (`local-`id, `run_id`, `session_id`,
   `pending`, tool) **before** the tool runs. Session → `waiting_input`.
2. In SQLite (WAL) → the *question* survives if the agent dies, keyed by `run_id`.
3. Human decides (UI or webhook). `DecideApproval` updates the row **and**
   correlates back into `conversation_events`.
4. On decision the session un-blocks/relaunches; `run_id` re-attaches the answer
   even across a restart. (Factor 5: unify execution + business state.)

## 5. State as a reducer (partial, deliberate)
- **Append-only event log** `conversation_events` (AUTOINCREMENT id, monotonic
  `sequence` per `claude_session_id`, immutable content; status via *columns*
  like `is_completed`/`approval_status`, not row edits). History = a fold over
  the log up the parent chain (Factor 12).
- **Mutable session rows** hold derived/cursor state (`status`, `last_activity_at`,
  `cost_usd`, `num_turns`, `result_content`) — a cache rebuildable from the log.
Log = source of truth; session row = index. A few things (`claude_session_id`,
creds, status) stored out-of-band because they can't be derived.

## What to adopt
Our current state already has `SharedStore`, `LfdId` newtypes, `Session{id,
wave_id,run_id,parent_session_id}`, `Run{parent_run_id}`, `StartupRecovery`,
`pending_activations`, `wait_for_session_and_resume`. Missing: the **durable
thread handle** and the **event log**.

1. **Continue = fork to a new row, never mutate.** Resume mints a new Session/Run
   id with `parent_*` set. Make the runtime honor the parent fields instead of
   reusing a live object → free branching/audit/replay.
2. **Add a `claude_session_id`-equivalent (`agent_thread_id`) to `Session`** —
   durable handle to the agent's own conversation; without it, resume-after-
   restart can only cold-respawn.
3. **Keep `run_id` as the approval/pending correlation key** — key durable
   pending records on it, reconcile on decision; don't hold the block in
   `WaveRuntime` memory.
4. **Introduce an append-only event log; `WaveRuntime` becomes a reducer** that
   replays on boot. Mirror `conversation_events`; reconstruct by folding up the
   `parent_*` chain. Biggest change; buys resume-after-restart.
5. **Split log (truth, append-only) from rows (mutable projection: status/cursors/
   cost).** Don't chase pure factor-5 — keep out-of-band handles as columns.
- **Encode explicit `can_transition(from,to)` guards + a startup janitor** for
  stuck transient states (they lacked it → wedged in `interrupting`). Give
  `StartupRecovery` this job. Skip `draft`/`discarded` until there's a UX.

**Minimal durable model for resume + fork:**
```
waves(id, status, ...)                             -- mutable projection
runs(id, wave_id, parent_run_id, status, ...)      -- mutable projection
sessions(id, wave_id, run_id, parent_session_id,
         agent_thread_id, status, tmux_name, ...)   -- mutable projection
events(id AUTOINCREMENT, session_id, sequence, type,
       content, tool_*, approval_status)            -- append-only, source of truth
pending(id, run_id, session_id, kind, payload, status) -- durable blocks
```
Boot = load projections, replay/verify vs `events`, resolve stuck transients,
re-attach `pending` by `run_id`. Resume = new session/run row + `parent_*` +
`agent_thread_id` to the executor with a fork flag.

Files: `hld/session/manager.go`, `hld/session/types.go`, `hld/store/sqlite.go`;
ours: `types/{session,wave}.rs`, `executor/wave/mod.rs`.
