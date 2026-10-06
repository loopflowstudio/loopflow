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

- **Steers per step (October 6).** On the FlowExec pass dropping per-node
  steer acknowledgement: "This seems potentially bad as that was specifically
  added to address some runaway token counts". Repair is slice 3 in the
  design; the mechanism there (a public newer-than option the driver passes)
  is the agent's.

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

The agent's readings of these points, since built, are at
`b30b193a0:scratch/questions.md`.

## Asked of Jack, unanswered; the pass took the default

- **Backgrounding** (answered above): `task create --run/--flow` is deleted;
  Desktop starts `lf task run` as a child, watches three seconds for a refusal
  and leaves it running.
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
- **Steers.** Per-node acknowledgement is gone; slice 3 restores the saving
  (below).
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
  slice 6 draws the workflow.
- **Test count.** The slice list said six tests; the build order at
  `26279a3d2:scratch/task-workflow.md` lists five behaviors, each covered.
  The list now says five.
- **Reaching `end` removes nothing:** the Task stays ready and
  `lf task complete` stays separate (draft default 3).

## Choices the Loops pass made without Jack

The agent's, October 6, building slice 2. None is confirmed by Jack Heart.

- **Authored form.** `- loop: <target>` is a flow item, with an optional
  sibling `step:` (a skill name or mapping) and nothing else. It is allowed
  in a Flow body and in an XOR path's `steps`.
- **The occurrence name is the existing `id`** on a `step:` mapping; no new
  field. A target matches an `id` first, else the one preceding step running
  that skill. Two or more are an error that lists their positions.
- **Scope.** A target is looked up among the steps before the loop in its own
  Flow body or XOR path, composed Flows included, so a Flow composed twice
  resolves each copy's loops to that copy.
- **Captured form.** The compiled step keeps how many steps back it returns,
  not a name; deciders need no id, and return counts are keyed by the
  decider's position in its body. Wire shapes are unchanged; a builtin
  graph's nodes now carry `id: null`.
- **A malformed captured distance is no loop.** One `return_target` reads the
  edge everywhere; a distance of zero or past the body's start, which the
  loader cannot produce, means no edge, so Iterate there is an error.
- **Old spelling fails loudly.** `repeat:` on a step is refused with a message
  naming `loop:`, so another repository's Flow cannot silently lose its loop.
  A `human: true` decider is refused.
- **Done-when grep.** Left matching: release notes, dated `performance/`
  reports and baselines, other Waves' memory (shipped or dated records), the
  test of the refusal above, and an unrelated `repeat:` parameter in
  `scripts/context_ablation.py`. That script now replays `loop-or-next`
  launches, so records made under the old name are skipped.
- **Website example** changed only its loop lines; its human steps wait for
  the docs slice.
- **Outside the slice.** The three-loop test ran into a Codex close that
  failed when the provider process had already exited ("does not own its
  process group"), about two runs in five. `close_engine` now treats a
  process that is gone as closed.
- **Seen, not fixed.** `scripts/check_architecture.py` reports
  `task_workflows` and `task_workflow_traversals` missing from the map
  (slice 1's tables; the docs slice's check). Five lib tests
  (`journal::boot_time`, three `ops::chapter`, one `ops::ci_watch`) fail in a
  full parallel `--lib` run and pass alone.

## Choices the Review repairs pass made without Jack

The agent's, October 6, building slice 3. None is confirmed by Jack Heart.

- **The option is global `--steers-after STEER`,** a steer's id (the Task
  event id shown as `steer:N` in context reports). It limits the Task
  direction any run in a Task checkout is given; it changes nothing without a
  Task. `lf context` ignores it.
- **What the driver passes.** Before each skill step it reads the Task's
  newest steer and remembers it for that node, in memory. A node's later run
  gets `--steers-after` its previous run's value; a `--steers-after` on the
  Flow launch is the floor for every step. Operation steps get none.
- **A steer can arrive twice, never zero times.** The step refreshes Linear
  after the driver's read, so a steer that lands in that gap reaches that run
  and the node's next one. Reading after the step instead would drop steers
  that arrive mid-turn and are not injected live.
- **Corrections** (`session resume`) carry no option; they add no seed.
- **An edge that runs nothing is named by the stage it enters:**
  `lf task run ISSUE end`. A Flow named like the stage on another edge from
  the same stage is refused at load as two edges with one name. A bare
  `lf task run ISSUE` still takes a stage's only edge, flowless or not.
- **`end` skips the unknown-name check** made before placing a worktree; the
  Task's stage decides whether it names an edge.
- **Tests live in `flow_tests`,** which has the Codex stand-in a headless
  Task run needs: one drives builtin `research` to `end`.

## Choices the Task primary pass made without Jack

The agent's, October 6, building slice 4. None is confirmed by Jack Heart.

- **The Task names its primary.** `tasks.primary_session_id`, not a scope
  mark on the Session row, after Jack's "just a smaller wrapper around this
  that saves that id in a field". The conversation stays a Task member and
  `primary_scope` on the wire stays null for it, so Desktop's grouping is
  untouched. Nothing on the wire says which conversation is primary yet;
  slice 6 decides how Desktop reads it.
- **Command.** `lf session ensure --task ISSUE` returns the primary;
  `--choose SESSION` names one of the Task's unfinished interactive
  conversations and is refused for any other Session.
- **Selection is remembered.** The first `ensure` records its pick (the only
  one, else the most recently used); later use of another conversation does
  not move it. A finished primary gives way to the same selection again.
- **Only an empty Task gets a new conversation,** running `task/session` in
  the Task's checkout and started like a repository's or Wave's primary. A
  Task with no checkout is refused. Selecting an existing one launches nothing.
- **`lf session replace` works on a Task primary:** it finishes that
  conversation and starts a fresh one. A repeat naming the replaced
  conversation is refused, unlike a repository's or Wave's. Desktop's Ctrl-C
  is unchanged, since it never sees a Task scope mark.
- **`lf resume` is unchanged:** it ranks every interactive conversation in
  the checkout, finished ones included, and ignores the primary.
- **Outside the slice.** `docs/lf-reference.md` was regenerated and picked up
  earlier passes' flags (`commit --push`, `--accept-unknown-exec`).

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
