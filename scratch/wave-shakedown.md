---
requires: wave-agent-design.md
produces: the live-vendor shakedown runbook for the wave agent MVP
---
# Wave agent shakedown

Code-complete ≠ done. Everything past the conformance traces is untested
against live vendors. Walk this in order; each step gates the next. Expect
2–5 real bugs — that's the point of the walk.

## 0. Build + unit sanity
- [ ] `cargo build --release` + `swift build`; full suites green.
- [ ] `lfd` running (main repo, per concerto-main-repo convention).

## 1. First contact — the mind alone (no lfd needed)
- [ ] From `../loopflow.goals`: `lf wave goals`.
- [ ] `.wave-endpoint` written; `GET /health` shows `idle`, then `turning`.
- [ ] Journal exists: `.lf/journal/waves/goals/journal.jsonl` — ThreadStarted
      first, then MindState idle→turning, TurnStarted….
- [ ] Watch the first real codex app-server turn stream through
      `GET /conversation` — items arrive, turn finalizes.
- Likely first bugs: app-server handshake details the traces missed; thread-id
  capture timing (the 10s window); prompt size on the first seeded turn.

## 2. Chat
- [ ] `POST /messages {op: "message", text: "what are you working on?"}` while
      idle → turn starts immediately, TurnStarted.answers names the message.
- [ ] Send a message mid-turn → queued; boundary turn drains it.
- [ ] Restart `lf wave` mid-conversation → thread intact (journal replay),
      turn ids continue; vendor thread cold-starts (documented).

## 3. Steer + interrupt (the new muscles)
- [ ] Mid-turn `{op: "steer", ...}` → lands in the live turn (codex
      pending_input); journal shows TurnSteered.
- [ ] Mid-turn `{op: "interrupt", text: ""}` → turn finalizes `interrupted`
      (not failed), state walks Turning→Interrupting→Idle, no orphan codex
      (`ps`).
- [ ] Interrupt & send: `{op: "interrupt", text: "do X instead"}` → next turn
      answers it.
- [ ] Pathological: interrupt during codex's own tool call — does the 10s
      deadline force-path fire cleanly?

## 4. Concerto
- [ ] WaveChat attaches via .wave-endpoint; in-progress turns stream (cursor
      visible); composer verb follows state (Send / Steer / Interrupt & Send /
      Interrupt).
- [ ] Kill the wave server → banner/composer degrade sanely; restart →
      reconnect, no transcript interleave.

## 5. Orchestration (needs lfd)
- [ ] Wave server registered: mind visible in the wave agent tree; second
      `lf wave goals` refused; loop_ticker skips the served wave.
- [ ] Ask the mind (via chat) to dispatch a worker → it runs
      `lfq worker run … ` → worktree named `loopflow.goals.<id>`, tmux session
      attachable from Concerto.
- [ ] WorkerDispatched/Finished appear in the journal (observation tail);
      next heartbeat turn's context carries <in_flight>.
- [ ] Bare `lf <flow>` inside the wave context self-registers as a child
      session (agent tree shows it).
- [ ] Worker's PR lands; Asana task moved via the mind (`lf op pm update`).

## 6. Soak
- [ ] Leave it grinding an hour: heartbeat cadence sane, MEMORY.md gets
      *curated* edits (not blobs) or the operating prompt needs tuning,
      journal growth reasonable, no fd/process leaks, spend acceptable
      (each heartbeat burns a subscription turn).

## Known-accepted gaps (don't re-find these)
- Vendor thread resume = cold start (visible thread survives via journal).
- SSE terminal-frame drop on a lagged client shows `running` until reconnect.
- WorkerFinished.summary is thin (exit + PR url) until worker reports exist.
- Steer degrades to queue on claude/opencode minds (codex-only for now).
- No Decisions/gating; ApprovalPolicy is AutoApprove.
