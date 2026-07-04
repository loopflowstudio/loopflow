# 12-Factor Agents → loopflow wave server

Source: github.com/humanlayer/12-factor-agents (Dex Horthy). All 12
`content/factor-*.md` read directly.

## The framing tension
12-factor's spine: **the "agent" is a thin, stateless `reduce` step; your
deterministic code owns the thread, context assembly, control flow, and tool
dispatch — at the token level.** A tool call is "just a model outputting JSON
describing what deterministic code should do" (F4); the loop is your `switch` over
that JSON (F8).

Our wave server draws the ownership boundary **one level higher**: we own the
*outer* loop (`WaveExecutor` sequences runs, watches git, fires listeners/crons,
updates SQL status) but **rent the inner loop to `codex exec`** — an opaque agent
that assembles its own context, runs its own tools, manages its own retries in a
tmux session. Defensible (F10 blesses "agents are one block in a mostly
deterministic system"), but factors *about the inner loop* (3, 4, 8-at-tool-
granularity, 9, 12) we've **delegated away** and can only influence via the prompt
+ process boundary. That's where most violations live.

## The 12 factors against our design
- **F1 NL→tool calls** — barely applies at our layer (codex does it). Applies to
  our HTTP intake (POST /messages, hooks → structured wave mutations). Fine.
- **F2 Own your prompts** — **strong.** `engine::load_goal` + `engine/prompt.rs`
  (3.6k lines), GOAL.md as prompt-as-code. Risk: codex's own system prompt/frame
  isn't ours — we own the payload, not the whole prompt.
- **F3 Own your context window** — **deep gap.** We assemble `GOAL.md + MEMORY.md`
  and hand it to codex; **codex owns the context from there** (file reads, tool
  results, compaction invisible to us). We do F3 for the *seed* only. Concrete:
  (a) MEMORY.md is an **unstructured markdown blob** — F3 wants structured,
  token-dense, tagged context; a free-form growing file is the "just dump the
  messages" anti-pattern in slow motion. (b) We can't do F3 error/safety
  formatting (compact a stack trace, redact a secret) — tool results never pass
  through our code.
- **F4 Tools = structured outputs** — **we don't own this** for wave work; codex
  executes its own tools. Our F4 surface is worker-launch (`/waves` routes).
- **F5 Unify execution + business state** — **the factor we most violate.**
  Three-way split: execution → SQL store; business → MEMORY.md (narrative) *and*
  git commits (work product); codex's per-pass thread → thrown away on tmux exit.
  F5's payoff (serialize, resume-any-point, fork, one debuggable history) we lack
  — reconstruct "what happened & why" = join SQL + MEMORY.md + git log + (lost)
  transcripts.
- **F6 Launch/pause/resume** — **launch strong; resume weak; pause absent.** Launch
  is excellent/multi-source. But a pass is a tmux `codex exec` process that runs to
  completion or dies; `resume_run_execution`/`recover_startup` = crash recovery at
  *run* granularity; **no pause/resume inside a pass** (F6's headline — pause
  between tool selection and invocation — is impossible; it's inside codex). An
  in-flight pass can't durably suspend to wait on a human/external op; we can only
  kill + re-seed.
- **F7 Contact humans with tool calls** — talk-only chat = right instinct, wrong
  shape. F7 wants human-contact as a *typed output that gates control flow*
  (`request_human_input` event that pauses + resumes on answer). Ours is out-of-
  band; the agent can't block on it. **Highest-leverage feature to add; blocked on
  F6 (durable pause).**
- **F8 Own your control flow** — **our thesis + best factor, at wave granularity.**
  `WaveExecutor` is a hand-owned outer loop with git-state polling as a control
  signal. But F8's sharpest bit is granular interruption *inside* a pass — opaque
  to us. We own control *between* passes, not *within* one. F8 pushes toward
  shrinking each pass toward a single action (→ F10).
- **F9 Compact errors into context** — codex does it *within* a pass; **we do
  nothing across passes.** On `fail_run` the error goes to SQL status, not
  compacted into the next pass's seed. Cheap high-value add: feed prior failure
  summary into next seed. Also no consecutive-error cap at the wave loop.
- **F10 Small focused agents** — **partial + a risk.** Wave-as-supervisor is F10's
  "mostly deterministic system"; progress-arm vs talk-arm is good F10. But a single
  `codex exec` pass is **not** bounded to 3–20 steps — a full agent on a broad
  GOAL = the monolithic shape F10 warns against, nested in our clean loop. Make
  passes narrower/more numerous → smaller context + more decision points.
- **F11 Trigger from anywhere / outer-loop agents** — **strong + a
  differentiator.** Triggers, listeners, crons, hooks = textbook. Missing half:
  escalate *to* a human (blocked on F7/F6).
- **F12 Stateless reducer** — **we are not a fold, and this is the lens unifying
  F5/F6.** A wave's state isn't derivable by folding an event log — scattered
  across SQL + MEMORY.md + git, per-pass transcript discarded. If the wave *were* a
  reducer over one persisted event thread, resume/fork/replay/unified-debugging
  fall out for free. Most structurally important idea for a long-lived, reactive,
  resumable server.

## Top design changes (most important first)
1. **Make the wave a fold over a single persisted event log (F12/F5/F6).** One
   append-only `wave_events` thread as source of truth (goal-set, pass-dispatched,
   commit-observed, run-failed, question-raised, human-answered, listener-fired,
   memory-updated). Derive status/"what happened"/resume points from it. **Unlocks
   3–6 below.**
2. **Durable pause/resume at the run boundary, keyed on events (F6/F8/F7).** A run
   enters `waiting` (on human answer / webhook / sibling wave) and resumes by
   appending a resume event — not kill + re-seed. Make the *pass* the atomic unit,
   the *event log* the resume mechanism.
3. **Human contact = a first-class typed event that gates control flow (F7/F11).**
   Replace side-channel chat with `human_input_requested` (typed: question,
   options, urgency) that pauses the run, surfaces via chat/SSE, resumes on
   `response_from_human`.
4. **Structure the seed context; stop the raw MEMORY.md blob (F3).** Assemble each
   pass's context as tagged token-dense sections (`<goal>`, `<wave_memory>`,
   `<recent_commits>`, `<last_failure>`, `<open_questions>`) rendered from the log.
5. **Compact prior-pass failures into the next seed + consecutive-error cap (F9).**
6. **Bound each codex pass; more, narrower passes (F10/F8)** — smaller context,
   cheaper failures, more outer-loop decision points.

**Keep as-is (aligned):** owned outer loop (`WaveExecutor`, F8); prompts-as-code
(GOAL.md/`engine/prompt.rs`, F2); multi-source launch + trigger/listener/cron
(F6-launch/F11); progress-arm vs talk-arm split (F10).

**Through-line:** we nailed owning the outer loop + triggering from anywhere, and
deliberately rented the inner loop to codex. The debt is all in **state
unification + resumability** — because we rented the inner loop *without* first
making the wave a fold over an owned event log. Fix #1 and the rest is incremental.
