# Open choices — October 5, 2026

For [the design](focus-on-your-own-work.md). The pass followed each default
below; defaults and interpretations are the agent's, not Jack Heart's. Earlier
interpretations are at `6f246fda4:scratch/questions.md`.

## Asked of Jack, unanswered; the pass took the default

- **Who backgrounds, now that `-b` blocks?** `task create --run/--flow` is
  deleted; skills tell the agent to background `lf task run` with its own tool;
  Desktop starts it as a child process, watches three seconds for a refusal and
  leaves it running.
- **Planning refusal** (terminal, moved or removed Task) applies to every Flow
  launch that resolves to a Task. It does not apply to a single skill or an
  interactive conversation in a Task worktree: binding stays allowed for done
  Tasks.
- **`task automate`/`automation`, `task interrupt`, `task wait`** are kept as
  ordinary Task commands.
- **The Task entry is `lf task run ISSUE [FLOW]`.**
- **A past Flow after its YAML changed** is drawn from its Exec sequence.
- **`lf commit`:** `-p` pushes; plain commit stays local.

## Choices this pass made without Jack

- **Steps re-resolve their skill by name.** The driver no longer hands a step
  captured skill text; a skill file edited mid-Flow takes effect at its next
  step. This follows "nothing extra on top of `lf flow`".
- **Claude steps without a required answer use the ordinary headless path.**
  Only deciding and routing steps use the stream-json harness. Before, every
  Flow step did.
- **A killed driver leaves a running agent turn to its Session.** Nothing reads
  that turn back into the Flow.
- **A watched landing ends the step with exit status 75** and the driver exits
  non-zero; Task status reads that as stopped at a watched landing, not failed.
- **Agent preflight before placement is gone.** An unavailable provider or
  account now fails at the first step, in the foreground.
- **Launch-time PR preparation is gone.** Publication refuses a settled PR and
  names `lf pr next`.
- **`LF_FLOW_ID`** (the driver Exec id) replaces `LF_FLOW_STEP` in a step
  agent's environment so the S1–S5 skills can still share one note per Flow.
- **Task Started for an operation-only Flow** is its operation step Exec in the
  Task checkout.
- **Flow states** are `current`, `completed`, `stopped`; `replaced` is gone.

## Interpretations still awaiting Jack's review

- Builtin `feature` ends at a published PR with no design-review pause; `ship`
  lands after the demo; `code` equals `pursue`. Open: whether `feature` should
  stop after kickoff, and whether `code` should remain.
- Main's `lf flow end` (#1435, LOO-326) is dropped: a stopped Flow blocks
  nothing. Open: whether Infrastructure wants a mark on stopped history.
- The per-Task automation on/off field is kept for CI repair only.
- Implementation defaults for unbuilt slices: 120-second Waiting fallback,
  most-recent interactive Task primary, optional occurrence names,
  backward-target loops, source-only workflow YAML.
- Global `-w` and `lf session open`, also removed by #1356, are left alone as
  outside this Task.
