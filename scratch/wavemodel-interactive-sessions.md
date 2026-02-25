# Interactive Sessions in Flow Execution

Converge the wave executor and session API so interactive steps run as sessions, not terminals.

## Problem

Two parallel paths exist for running interactive work:

1. **Executor path**: Detects `interactive: true` → creates agent record with `Waiting` status → broadcasts `wave_waiting` → Concerto launches embedded terminal running `lf {step}` → terminal subprocess runs a completely separate agent → exit code signals completion.

2. **Session path**: Concerto creates a session via `POST /v0/sessions` → harness starts → SSE event stream → chat UI. Used only for StartWaveView design NUX.

These paths don't talk to each other. The executor can pause at an interactive step and resume after, but the interactive step itself runs in a terminal with no connection to the session API. The session API can run steps interactively, but has no connection to the executor's flow progression.

This means:
- Flows can't mix auto and interactive steps seamlessly. After `design` finishes in the terminal, the user manually runs `implement` from StepRunner.
- The chat-based session experience (richer than a terminal) is only available at wave creation time.
- There's no way for lfd to auto-commit and advance after an interactive step without the terminal convention of `lf {step} && lf ops commit --push`.

**Who benefits**: Every user running multi-step flows. The default NUX flow (`design → ship → review`) requires this. The steady-state flow (`ingest → kickoff → review-design → ship → review`) requires this.

**Why now**: All dependencies shipped. Session API (agentapi phases 01-03), design-first onboarding (wavemodel phase 03), and shared prompt assembly (`prepare_step_prompt()`) are in place. The two paths share enough infrastructure to converge without a rewrite.

## Approach

**Executor creates sessions. Concerto joins them.**

When the wave executor hits a `WaitInteractive` step:

1. **Executor calls `SessionManager::create_session()`** with the step's config, wave_run_id, and run mode `"interactive"`. The session starts the harness immediately — `prepare_step_prompt()` builds the system prompt (context, docs, diff) and task prompt (step instructions). The harness begins executing the step.

2. **Executor stores session_id on the wave run** and sets wave status to `Waiting`. The `wave_waiting` WebSocket event includes the `session_id`.

3. **Executor spawns a watcher task** that monitors session status. When the session transitions to `Ended` or `Failed`, the watcher fires.

4. **Concerto receives `wave_waiting` with `session_id`**. WaveDetailPanel creates a ChatState that joins the existing session (skips `ensureSession()`, uses the provided session_id directly). WaveChatView renders the session transcript and input.

5. **User interacts with the session.** The agent is already running the step. For steps like `review`, the agent reads the diff and presents findings — the user responds. For steps like `design`, the agent asks what to build — the user describes. Either way, it's a conversation.

6. **User ends the session** (explicit "done" or ChatState `endSession()`). Alternatively, the agent completes naturally (all turns used).

7. **Executor's watcher fires.** Auto-commits changes in the worktree. Advances `step_index`. Resumes the execution loop with the next step.

The harness starts immediately because the step prompt *is* the instruction. Interactive steps like `design` say "Help the user design..." — the agent will ask for input. Steps like `review` say "Walk through the diff..." — the agent starts working and presents results. The user participates either way.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Executor signals, Concerto creates session | Concerto owns session lifecycle; executor needs a "continue" endpoint for Concerto to call after session ends. | Splits ownership. Executor can't guarantee flow progression if Concerto crashes or disconnects. |
| Keep embedded terminal, add session later | Simple, no protocol changes. Terminal works today. | Doesn't converge the paths. Terminal-based interactive steps can't benefit from session features (transcript, SSE streaming, reconnection). Flows still can't auto-advance. |
| Add a `Pending` session state, start harness on first user input | Avoids running harness with nobody connected. Saves compute. | Over-complicated. The harness starting immediately is fine — interactive steps produce useful output before the user arrives (review reads the diff, design greets the user). The user can always reconnect via replay. |

## Key decisions

**Executor owns session lifecycle, not Concerto.** The executor creates the session, and the executor's watcher decides when to advance the flow. Concerto is a viewer/participant, not the controller. If Concerto disconnects, the session keeps running and the user can reconnect. If the session ends while Concerto is away, the executor still advances.

**No new session states.** The existing `Starting → Active → Ending → Ended/Failed` lifecycle works. The executor just creates a session and watches for terminal states. No `Pending`, no `WaitingForUser`.

**Harness starts immediately.** The step prompt is a complete instruction. The agent begins working on arrival, not when the user sends a message. This matches how auto steps work — the only difference is the agent can receive user input during execution.

**ChatState joins, doesn't create.** When Concerto sees `wave_waiting` with a `session_id`, ChatState skips `ensureSession()` and connects to the existing session via `streamSessionEvents()`. The session already has a transcript (from harness startup and initial agent output), so replay delivers it.

**Embedded terminal becomes fallback.** InteractiveSessionView (Ghostty terminal) stays in the codebase but is no longer the primary path for executor-driven interactive steps. It remains available for manual `lf {step}` runs from StepRunner or CLI.

**Default NUX flow is `design → ship → review`.** This is the first flow to exercise the new path. Three steps: interactive design, auto ship (implement → compress → gate → consolidate), interactive review.

## Scope

**In scope:**

- Rust: WaveExecutor creates a session via SessionManager on `WaitInteractive`
- Rust: Session completion watcher triggers auto-commit and flow advancement
- Rust: `wave_waiting` WebSocket event includes `session_id`
- Rust: Wave run stores `session_id` for the current interactive step
- Swift: WaveDetailPanel uses ChatState (not InteractiveSessionView) for executor-driven interactive steps
- Swift: ChatState supports "join existing session" mode (session_id provided externally)
- Swift: WaveEvent model adds `sessionId` field
- Default NUX flow: `design → ship → review`

**Out of scope:**

- Multi-session per step (parallel agents within one interactive step)
- Session handoff between users
- Removing InteractiveSessionView entirely (stays as manual/fallback)
- Per-tab step routing in Concerto (still defaults to step: design for user-initiated chats)
- Real-time wave content refresh via filesystem watcher
- Database schema changes for wave content

## Done when

1. `lf flow design-and-ship` runs via lfd: `design` surfaces as a chat session in Concerto, `implement` and subsequent steps run headless, `review` (if in flow) surfaces as a chat session
2. `wave_waiting` events include `session_id`; Concerto joins the session without creating a new one
3. Interactive step completion triggers auto-commit and flow advancement — no terminal, no manual StepRunner action
4. Reconnection works: close Concerto during an interactive step, reopen, session transcript replays and interaction continues

Wave goals advanced: "Concerto orients new users toward design, not configuration" — the default flow makes this automatic. "Prompt assembly shared between executor and sessions" — already shared, now exercised by executor-created sessions.
