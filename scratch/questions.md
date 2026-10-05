# Open choices — October 5, 2026

For [the design](focus-on-your-own-work.md). The pass followed each default
below; defaults and interpretations are the agent's, not Jack Heart's. Earlier
interpretations are at `6f246fda4:scratch/questions.md`.

## Answered by Jack Heart (October 5)

- **Who backgrounds, now that `-b` blocks?** "cant you just background with
  &" and "like llms know how to background things". The caller does; there is
  no detached launch mode. Explicit `nohup`/`&` examples in skills are fine.
  The six builtin skills that say "background it with your own tool" stay.
- **Task helpers.** On deleting `task create --run`: "Im fine with this. i am
  fine with task helpers that sit on top of the lf system, but i dont want to
  introduce parallel paths or drivers." A Task command may prepare, then it
  enters the ordinary path; it never owns a second launcher, driver or record.

## Asked by Jack, open

- **How a step learns and records its position.** At `45ca0d2ac` the driver
  passes a JSON `FlowStep` as the hidden `--__flow-step` argument (the hidden
  `__flow-step` command for operations), and the Exec's recorded argv is the
  record. Jack: "Can we do this environemnt variables? I hate __ and hidden
  arguments." Proposed, not decided: the driver sets one environment variable;
  the step's `lf` consumes and removes it at startup, as it already replaces
  `LF_PROCESS_ID`, so nested commands never inherit it; the Exec row gains one
  nullable field for flow, sequence, label, graph key and iterations, because
  Execs record argv and not environment. Step Execs then read as plain
  commands (`lf -b implement`, `lf task sync --plan`).

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

- **A step runs the skill its driver compiled.** It looks the skill up by
  name; the driver puts its compiled skill on the step's argv only when that
  lookup would differ (a step override, a router, a file edited mid-Flow).
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
