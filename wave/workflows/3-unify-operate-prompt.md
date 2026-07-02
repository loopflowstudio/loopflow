# Unify the operating prompt

**Finish line:** One operating prompt (`LOOPFLOW.md`) reaches every agent that
should route work through `lf` — launch prompts, `loopflow.goal`'s loop session,
and lfd-launched sessions. No second copy of the guidance, no caller
reconstructing system sections by hand.

## Context

The docs-flag work renamed and flipped the operate concept:

- `OPERATE.md` → `LOOPFLOW.md` (`engine/builtins.rs:7`, `LOOPFLOW_DOC`).
- `<lf:operate>` → `<lf:loopflow>` in the rendered prompt.
- The `--operate` opt-in flag became a **default-on** `--no-loopflow` opt-out for
  launch prompts. `GatherContextOpts` still carries `operate: bool`, set from
  `!no_loopflow` at the launch/CLI boundary (`engine/launch.rs:103`,
  `bin/lf-prompt.rs:61`).
- lfd sessions now default loopflow guidance **on**: `sessions/mod.rs:289`
  hardcodes `no_loopflow: false`. It's still set locally in `SessionManager`, not
  a wire field.

`format_system_sections` (`engine/prompt.rs:1626`) still owns system-safe
rendering; vendor skill seeds reuse it so surface, voice, and loopflow sections
stay in sync.

Two threads remain from the original unification, reframed by the default flip:

- **`loopflow.goal` still carries its own inline prompt.** The goal-branch loop
  session injects `LOOPFLOW_OPERATING_PROMPT` (`engine/flow.rs:161`, used at
  `flow.rs:366`) rather than composing from `LOOPFLOW_DOC`. That const is the
  "Looping Agent for this Wave" orchestrator guidance — decide whether it folds
  into `LOOPFLOW_DOC` or stays a distinct goal-loop layer, and retire the
  duplication either way. The const lives on `loopflow.goal`, so this is a
  cross-branch handoff.
- **lfd sessions can't opt out per-session.** Loopflow guidance is now on by
  default for lfd-launched runs, but there's no wire-level switch — `no_loopflow`
  never crosses the DTO boundary. If a wave-launched run needs to suppress it
  (or force it), decide required-vs-Optional under the DTO fixture discipline
  (`tests/fixtures/dto/`, round-trip in all three languages) before threading it.
  If launch-local default-on is the final answer, record that and close the thread.
- **The shared-path risk.** `format_system_sections` is the one place system
  sections are assembled. A future caller that rebuilds surface/voice/loopflow
  sections by hand would silently drift. Keep new callers routing through it.

## Done when

- `loopflow.goal`'s loop session composes from `LOOPFLOW_DOC` (or a documented
  goal-loop layer on top of it); no orphaned inline operating-prompt const
  survives.
- An lfd-launched run can control loopflow guidance over the wire (or the
  decision to keep it launch-local default-on is recorded), with DTO fixtures
  green in Rust, Python, and Swift if a field lands.
- No caller assembles system-safe sections except through
  `format_system_sections`.
