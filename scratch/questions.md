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

11. "i think maybe we call that like a TaskWorkflow or something ? might need
    to think about ones that dont end in land, multi-PR tasks or 0-PR tasks.
    (I am pretty confident we want 0 PR tasks; not sure we need multi-PR
    tasks)".

12. On the outer/inner two-record table: "your table looks great. lets start
    to center on tat".

13. On keeping this PR to the inner record and leaving the outer one as the
    next Task: "Yeah i think so." On the name: "FlowExec works fine".

Where the outer model already appears (searched October 5): the September 30
design (`bc78c27c0:scratch/growth-thoughts.md`: "interactive skills are
nodes; background Flows are the edges"), the October 4 kickoff
(`5090f672e:scratch/focus-on-your-own-work.md`), and open Tasks LOO-317 (Mac
shows a flow as only the steps that need you), LOO-297 (design or prototype
review paths), LOO-322 (reach demo and landing unattended) and LOO-367. No
Task owns TaskWorkflow. Conflict to resolve there: the September 30 design
and LOO-367 both record Jack saying there is no separate "Task workflow"
concept.

Resolution, centered on that table (the names TaskWorkflow, from Jack, and
FlowExec, the agent's, postdate it): the outer thing is TaskWorkflow and
is not built in this PR; one `lf` flow run gets an append-only driver record
named FlowExec.

The consolidated model is in [the design](focus-on-your-own-work.md)
under "Flow model".
Takeover and resume are not built: Jack's request for the FlowExec pass
excluded them along with TaskWorkflow.

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
- **A past Flow after its YAML changed** keeps the graph FlowExec captured at
  launch (the earlier pass redrew it from the Exec sequence).
- **`lf commit`:** `-p` pushes; plain commit stays local.

## Choices the FlowExec pass made without Jack

The agent's, October 5, while building FlowExec and oblivious steps. None is
confirmed by Jack Heart.

- **Shape.** `flow_execs(exec_id, flow, graph)` and
  `flow_exec_steps(seq, flow_exec_id, exec_id, node, iterations)`. Triggers
  refuse updates and refuse a step that is not its driver's child Exec. The
  step label is read from the graph, not stored.
- **Finding the child.** The driver polls for its newest child Exec every
  10 ms and then appends the step row. A driver killed in that gap leaves the
  step's Exec without a row. No Exec id is assigned by the parent.
- **Operations go through the ordinary CLI.** The in-process interpreter and
  its allowlist are deleted, so any `lf` command can be a `cmd:` step.
  `cmd: pr open` now opens the browser page, as the command does; `commit` no
  longer applies the managed-Task guard the interpreter added; `repo release
  run` under a cron firing reads the receipt from the existing hidden
  `--__cron-receipt`/`--__cron-lock-fd` globals, which stay. Old spellings in
  a Flow file (`task sync`, `rebase`) are translated by the driver.
- **Watched landing.** After an operation the driver stops the Flow when a
  landing of its checkout, touched since the step began, still awaits its
  merge. Exit status 75 is gone; Task status says "Flow stopped after …"
  instead of naming the watched landing.
- **The answer contract is message text only.** Provider structured-output
  requests are deleted for every provider, and Claude steps all use the
  ordinary headless path. The driver accepts the JSON value alone, in prose
  or in a code fence. How reliably real providers comply is unproven.
- **Correction command.** `lf -b session resume ID MESSAGE`, a new optional
  argument on the existing command. It reruns the conversation's skill prompt
  with the message under the provider's own history, as the old correction
  did. Without `-b` a message is refused.
- **A step looks its skill up by name.** A skill file edited mid-Flow applies
  to later steps. Of a step's authored overrides only `agent` travels, as
  `--model`, and only when the launch named none; `action_style` on a step is
  dropped. The default XOR router is now the builtin skill `xor-route`, which
  a repository can override.
- **Steers.** Every step receives all of its Task's steers, like any skill run
  in the checkout. Per-node acknowledgement, which spared a repeated node
  steers it had already seen, is gone.
- **`LF_FLOW_ID` stays,** set by the driver in the step's environment so the
  S1–S5 skills can share a note. `lf` reads it nowhere.
- **Started.** The driver marks its Task started when it writes the Flow row;
  the trigger accepts a Flow run from the Task's checkout as evidence.
- **Performed work.** The Exec filter counts every Flow step Exec in the
  Task's checkout; before, only operation steps.
- **Past Flows keep their launched graph.** The redraw from the Exec sequence
  is deleted. Loop counts for deciders outside the latest step's own path read
  zero; settled counts inside a finished XOR path are no longer shown.
- **Session position.** A Session's node and iterations are read from its
  step row. The draft drops `agent_sessions.node`/`iterations`; a saved Flow's
  position survives in its `legacy_flow` observation.

## Choices the TaskWorkflow pass made without Jack

The agent's, October 5, building slice 1. None is confirmed by Jack Heart.

- **`feature` and `code` are workflows only.** The builtin Flows of those
  names are deleted. `lf run feature` fails and names `lf task run <issue>
  feature`; it is not rerouted through the Task entry, inside a Task or out.
- **A repository Flow keeps its name.** `.lf/flows/feature.yaml` hides the
  builtin workflow `feature`, so a repository that customized it runs it ad
  hoc as before. `.lf/workflows/feature.yaml` wins over both.
- **Which Tasks get a workflow.** One whose Project default names a workflow,
  on its first `lf task run`; or one that names a workflow explicitly. A
  Project whose default is a plain Flow leaves its Tasks ad hoc.
- **Naming a workflow replaces.** `lf task run ISSUE WORKFLOW` on a Task with
  another workflow starts the named one at `start`; the earlier rows stay. A
  name that is both an outgoing edge's Flow and a workflow means the edge.
- **The traversal names the `lf task run` Exec,** which is the edge Flow's
  driver, so the row needs no FlowExec to exist yet. A launch refused after
  the row is written leaves a failed traversal: position unchanged.
- **Position.** At `to` only when that Exec succeeded. A driver with no exit
  record counts as running unless its process is known dead. A Flow that
  stops at a watched landing has not succeeded, so `ship` returns the Task to
  `demo` until a later run of it succeeds.
- **No guard on a running edge.** A second `lf task run` while one runs
  chooses among the edges leaving the running edge's `from`.
- **Edges accept what `lf run` accepts,** a skill included: `research` runs
  the `research` skill. Flowless edges are allowed only into `end`.
- **`research` names a workflow and a skill.** `lf research` stays the skill.
- **Wire placement.** The workflow rides in `TaskWork`, not the Task Flow
  snapshot, so the roadmap does not carry it; Desktop reads it per Task.
- **Guidance** is appended wherever a Task binding's context is rendered
  (`resolve_work_binding`), headless launches included.
- **Skill prose.** Only `advance` and `launch-plan` were corrected for the
  deleted Flows; other skills still say `lf task run <issue> <chosen-flow>`,
  which a Task on a workflow refuses unless that Flow leaves its stage.
- **Flow catalog.** `lf flow list` no longer lists `feature` or `code`, so a
  Project default of `feature` has no Flow graph for Desktop to draw until
  slice 5 draws the workflow.
- **Test count.** The slice list said six tests; the build order at
  `26279a3d2:scratch/task-workflow.md` lists five behaviors, each covered.
  The list now says five.
- **Reaching `end` removes nothing:** the Task stays ready and
  `lf task complete` stays separate (draft default 3).

## Choices the earlier pass made without Jack

- **A killed driver leaves a running agent turn to its Session.** Nothing reads
  that turn back into the Flow.
- **Agent preflight before placement is gone.** An unavailable provider or
  account now fails at the first step, in the foreground.
- **Launch-time PR preparation is gone.** Publication refuses a settled PR and
  names `lf pr next`.
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
