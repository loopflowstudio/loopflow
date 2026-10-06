# TaskWorkflow — draft for Jack Heart's review

October 5, 2026. LOO-353. Jack: "I think lets stick with TaskWOrkflow. I
think you own TaskWorkflow. We cant do this design correctly without it."
This draft is the agent's; nothing below the first section is decided. The
inner record, FlowExec, is in [the design](focus-on-your-own-work.md).

## Decided by Jack

- Two notions of Flow: "One where there there is a start and a land node and
  then in between are human sessions, and the edges are lf flows."
- The outer one is called TaskWorkflow and belongs to LOO-353.
- It is the mutable one: "the FlowSession is mutable, but the Flow exec is
  not"; "in the same way the session api lets you replace / take over any lf
  skill, the flow session api would let you replace / take over any running
  flow". (Said before the outer thing was renamed TaskWorkflow.)
- "might need to think about ones that dont end in land, multi-PR tasks or
  0-PR tasks. (I am pretty confident we want 0 PR tasks; not sure we need
  multi-PR tasks)".
- Standing rules it must obey: Task helpers sit on top of ordinary `lf`, with
  no parallel paths or drivers; no `session ready`/`complete` handshake or a
  renamed one; every `lf` flow run is tracked the same, ad hoc or not.

This reverses two earlier lines, deliberately: September 30 and LOO-367
("there is no separate 'Task workflow' concept") and October 4 ("without a
shared playhead"; "no runtime row"). TaskWorkflow executes nothing itself.

## Direction from Jack Heart — October 6 (after seeing it built)

On the built record being append-only with derived position:

- "its not essential we make everything accessible per se. However, i do
  think the data model and the basic data API should be well thought out and
  mutable and we should be clear what we want that mutable API to be."
- "a user will be using the Loopflow desktop app. They will have an open
  primary task session, which should have as much access to the workflow as
  possible. We likely want to give the user buttons to press; however it's
  possible that we want to have all the interaction come directly from inside
  the LLM session".
- "the right place to start is looking at the sort of human-decide loops. The
  demo, okay, send it back Okay this time it's good enough. Let's proceed
  towards landing."
- "there's no graph editing flow ... If you want to change the graph itself,
  you need to start over from a new graph"; "the graph is fixed upfront when
  it's loaded and never mutated".
- "We want both the human and the LLM to have direct control over choosing
  x-words [XORs] or choosing to go back in loops".
- "this is kind of like the complete button is what we had before and this is
  kind of a generalization of that but also hopefully less awkward ... it
  doesn't need to end the LM session that you're in".
- "a simple task CLI command for navigating through these workflows, which
  are essentially live states in the LFDB And then we can also make buttons
  in the Loopflow desktop UI that make the choice as well as show you where
  you are".
- On 0-PR Tasks still carrying a PR slot and not completing at `end`: "This
  seems deferable".

## Proposed mutable model (agent's, for Jack's review)

- **Live state.** One row per Task: the workflow's graph, fixed when loaded,
  and the Task's position, stored: at a stage, or on an edge with the Flow
  run carrying it. A separate append-only history records each move: who made
  it (a person, a conversation, an edge finishing), from, to, the Flow run,
  and a note.
- **Choose.** One command picks a way out of the current stage and, when that
  edge has a Flow, runs it; the note travels to the Flow as Task direction.
  This is today's `lf task run ISSUE <choice>`. The process running the edge
  writes the position when it ends: forward on success; otherwise the Task
  stays on the edge, shown as stopped.
- **Set.** One command puts the Task at a named stage without running
  anything: go back, skip ahead, or correct the record after work done by
  hand or a landing that settled later. This does not exist yet.
- **Start over.** Taking up a workflow by name replaces the graph and resets
  position; history stays.
- **Read.** Graph, position, the ways out and the history, for the
  conversation, `task status` and Desktop's buttons alike.

Open for Jack: whether choosing and running stay one command; whether a
stopped edge returns the Task to its stage or holds it on the edge; whether
an edge finishing in the background should reach the conversation by any
means other than the conversation's own background tool or reading status.

## Built — October 5, unreviewed by Jack

The agent's choices here are listed in [questions.md](questions.md) under
"Choices the TaskWorkflow pass made without Jack".

- **Loader.** `.lf/workflows/<name>.yaml`, else builtins `feature`, `code`,
  `research` (`engine/workflow.rs`). Rejected: an unknown or unreachable
  stage, a stage skill or edge Flow that does not load, a flowless edge that
  does not enter `end`, two edges from one stage running the same Flow.
- **Record.** `task_workflows` (Task, captured named graph; the newest row is
  the Task's) and `task_workflow_traversals` (edge, the Exec that ran it), in
  the one draft, both append-only.
- **Moving.** `lf task run ISSUE [FLOW]` takes the outgoing edge that runs
  FLOW, or the only one; names a workflow to take it up; refuses anything
  else, naming the edges. An edge with no Flow is recorded and returns.
- **Reading.** `task status --json` carries `execution.work.workflow`: name,
  stages, edges, position (`stage`, or `edge` with its driver Exec) and
  traversals. Text status prints one line. Swift decodes the same fixture.
- **Guidance.** A bound Task launch's context names the stage's skill and the
  command for each outgoing edge.
- **Check.** `cargo test -p loopflow --test task_flow_launch_tests` (5) and
  `--lib engine::workflow ops::task_workflow` (3, stage guidance included)
  pass; Swift `DTOFixtureTests` decodes the workflow. After compress (name
  column, workflow listing and authored-edge type removed): the same two
  commands, `dto_fixtures` 18 and all-target Clippy pass.

Desktop draws the workflow and starts its edges (slice 6); the catalog lists
workflows, and the Wave page sets the default and opens sources (slice 7). Docs describe
it (slice 8). Not built: a take-over command (open choice 2).
