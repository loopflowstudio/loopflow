# Unify the operate prompt

**Finish line:** One operating prompt (`OPERATE.md`) reaches every agent that
should route work through `lf` — the `--operate` flag, `loopflow.goal`'s loop
session, and lfd-launched sessions that opt in. No second copy of the guidance,
no caller reconstructing system sections by hand.

## Context

The ambient-agent-layer work made loopflow's operating guidance opt-in. The
always-injected `<lf:rlm>` block and the bundled fallback voice prompt are gone;
`OPERATE.md` is the single builtin source, injected as `<lf:operate>` only when
`lf --operate` is set. `format_system_sections` (`engine/prompt.rs:1626`) now
owns system-safe rendering, and vendor skill seeds reuse it so surface, voice,
and operate sections stay in sync.

That shipped. Three threads were deliberately left for later:

- **`loopflow.goal` still carries its own operating prompt.** The goal-branch
  loop session injects a `LOOPFLOW_OPERATING_PROMPT` const rather than reading
  `OPERATE_DOC`. Point `render_goal` at `OPERATE_DOC`, retire the const, and the
  `lf op` git/worktree guidance the goal prompt currently lacks comes for free.
  This is a cross-branch handoff — the const lives on `loopflow.goal`, not here.
- **lfd sessions can't opt into operate.** `lfd/sessions/mod.rs:289` hardcodes
  `operate: false`. `--operate` is a local launch flag today, off the lfd wire on
  purpose (no DTO churn). A wave-launched autonomous run that should operate
  through `lf` needs a wire-level switch — decide required-vs-Optional under the
  DTO fixture discipline (`tests/fixtures/dto/`, round-trip in all three
  languages) before threading it.
- **The shared-path risk.** `format_system_sections` is now the one place system
  sections are assembled. A future caller that rebuilds surface/voice/operate
  sections by hand would silently drift. Keep new callers routing through it.

## Done when

- `loopflow.goal`'s loop session renders `OPERATE_DOC`; no inline operating-prompt
  const survives, and the goal prompt gains the `lf op` guidance.
- A wave/lfd-launched run can request `operate` over the wire (or the decision to
  keep it launch-local is recorded), with DTO fixtures green in Rust, Python, and
  Swift if the field lands.
- No caller assembles system-safe sections except through
  `format_system_sections`.
