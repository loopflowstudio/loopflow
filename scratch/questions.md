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

## Step invocation — decided by Jack Heart (October 5)

At `45ca0d2ac` the driver passes each step a JSON `FlowStep` through the
hidden `--__flow-step` argument (the hidden `__flow-step` command for
operations); readers find Flows by searching recorded command text.

Jack: "Can we do this environemnt variables? I hate __ and hidden arguments."
The agent proposed an environment variable plus a nullable Exec field. Jack
rejected both halves:

1. "the skill is its own argument. we should use lf from flows in the
   'natural' way, invoking the right skill".
2. On needing a field for position: "right, but execs are supposed to trakc
   their parents already".

So a step is the command a person would type, started as a child of the
driver: `lf -b skill implement [message]`, `lf task sync --plan`. No
`FlowStep` payload, hidden argument, hidden command, position variable or new
Exec field. A Flow is read from the driver Exec, its children in start order
and their commands; iteration is the count of earlier siblings with the same
command.

3. "why does the skill lf exec even need to know what step position it is?
   just runt the right skill. it doesnt need to know its part of a flow to
   run".
4. "its fine if a flow process does extra logging in lfdb or the file system,
   or maintains state, to make querying about active/completed flows easier.
   but i think that sould happen in the flow, not the step".

5. "we should make the step part of a flow as radically simple as a normal
   flow, and put the flow logic in the flow process".

6. "to the extent that there was useful stuff going on with the flowsession
   before, it should be part of any lf flow. for now we drop any notion of
   'one primary flow' for a task, and we might add that back on as a layer on
   top once we just have generally 'you can run flows for any tasks, and we
   will do good tracking for any flow you run adhoc just the same as the
   standard operating protocol one that is run aautomatically when you start
   a task'".

What this settles: tracking is uniform. Every Flow run, ad hoc or the
Project's default started by `lf task run`, is tracked the same way; no Flow
is primary for a Task. A primary Flow may return later as a layer on top.

7. Confirming the reading below: "Yeah, we should be able to do taht stuff
   for any running flow. the flow driver should be responsible for maintaining
   the state to make that queryable and scalable and performant; the 'client'
   code of it should be made to work on top of any flow". And: "we *dont*
   right now need to make any flow like mutable on demand by some sort of API
   - read only other than the parent process ctrl-cing it or whatever".

Decided, then: the driver maintains queryable Flow state for every Flow;
readers (`flow list/show`, `task status`, Desktop) are clients of that state
and work for any Flow; the state is read-only to everyone but the driver. The
only control is ending the driver process.

What it reopened (agent's reading, since confirmed by point 7): `68c1ffc55` removed the
Flow record outright, and with it things that were useful for any Flow: the
graph as launched, each step's result and iteration, and a live position for
Desktop's graph. Point 4 allows the driver to keep exactly that. The next
loop should give every Flow a driver-written record (started with its
compiled graph; each step started and ended with its child Exec, graph key
and iteration; ended) and have `flow list/show`, `task status` and Desktop
read it. It carries no Task selection, no resume and no authority, and no
step reads or writes it.

8. On a record that extends the driver Exec: "its not exactly 1:1 with exec
   in the same way that a simple interactive skill session is not 1:1 with
   its launching process". Then: "ok so yeah maybe we still want FlowSession.
   And then the FlowSession is mutable, but the Flow exec is not".
9. "but just in the same way the session api lets you replace / take over any
   lf skill, the flow session api would let you replace / take over any
   running flow".

10. "i have also previously talked about the idea of there being two different
    notions of flow. One where there there is a start and a land node and
    then in between are human sessions, and the edges are lf flows". Then: "I
    think maybe the flowsession is *that*".

Open, and blocking the next pass: which thing FlowSession names.

The consolidated model is in [the design](focus-on-your-own-work.md#flow-model--decided-october-5-governs-the-rest-of-this-document).
Open: whether takeover is built in this PR or only designed for.

The rule: Flow bookkeeping belongs to the driver process. It may record what
it started and where it is (journal events, rows or files it writes itself).
A step is told nothing and writes nothing about the Flow.

Agent's reading of what follows, for the implementing pass to confirm in
source:

- The step looks its skill up by name when it runs. A mid-run edit to the
  skill file changes later steps. Step-level overrides become ordinary
  arguments or go.
- A deciding or routing step gets its answer contract in its message; the
  driver already validates the answer and corrects it. A correction continues
  the same conversation through the ordinary resume command.
- The driver finds its step's Exec and conversation as its own child, not by
  sequence number.
- Identifying a driver among Execs must not depend on re-resolving names
  later. `drive()` already journals Flow started/ended with the Flow name
  against the driver Exec; that is ordinary logging and can carry the name.
- `LF_FLOW_ID` (driver Exec id, for shared notes) is the step's parent Exec
  id; whether it stays is open.

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
