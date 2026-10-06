# TaskWorkflow — draft for Jack Heart's review

October 5, 2026. LOO-353. Jack: "I think lets stick with TaskWOrkflow. I
think you own TaskWorkflow. We cant do this design correctly without it."
This draft is the agent's; nothing below the first section is decided. The
inner record, FlowExec, is in [the design](focus-on-your-own-work.md).

## Decided by Jack, October 5

His words are in [questions.md](questions.md), points 8–13 under "Step
invocation": two notions of Flow, the outer one named and owned by LOO-353,
0-PR and multi-PR Tasks. Standing rules: Task helpers sit on top of ordinary
`lf`; no ready/complete handshake; every Flow exec is tracked the same.

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

## Workflow contract — four layers (agent's, for Jack's review)

**Name.** Jack, October 6: "lets just call TaskWorkflow Workflow". The
Task's live thing is a *Workflow*; the authored YAML it takes up is a
*workflow definition*. Slice 9 renames types, tables, wire fields, Swift
mirrors and docs to match, with no alias.

Jack, October 6: "lets use Edge and node instead of Stage and Way OUt". A
Workflow has *nodes* and *edges*: `nodes:` in the YAML, node in types, wire
fields, status text, errors, skills, Desktop and docs. "Stage" and "way
out" go, with no alias. Where this note still says stage, read node.

Jack, October 6: "we should make sure we understand what is the core data
model at the center of this, what is the data model <--> db API, db <--> cli,
db <--> swiftui". Slice 9 builds to this.

**Projects.** Jack, October 6: "Then projects have workflows instead of
default. project workflow and task worfklow both work and are the samle". A
Project names a workflow, not a "default Flow"; a Task may name its own; both
are the same definition and behave the same once taken up. Agent's reading:
the Project's `flow:` field becomes `workflow:`, a Task's own choice wins,
and a Project can no longer name a plain Flow as its default.

**1. Model.**
- *Workflow definition:* name, stages (name, skill), edges (from, to, optional
  Flow or skill; unique among the edges leaving a node). Authored YAML.
- *Workflow:* one per Task. The graph it took up, fixed from then on, and its
  position.
- *Position:* at a node, or on an edge with the `lf task run` Exec carrying
  it. That Exec may start the edge's Flow more than once (slice 9a), so one
  edge can have several Flow runs. Running or
  stopped is that Exec's own state, never stored here.
- *Move:* one history entry: when, who (a person, a conversation, an edge
  ending), what (took up, chose, arrived, set), from, edge, to, Exec, note.

**2. Model ↔ database.** Two tables in the one draft: `task_workflows` (Task,
graph, position; one row, updated in place) and `task_workflow_moves`
(append-only). Four store calls and no other SQL:
- read a Task's workflow with its history;
- take up a workflow: store the graph, position `start`, one move;
- choose an edge: only from the edge's `from` stage or from that stage's
  stopped edge, as one transaction, so two choosers cannot both leave;
- arrive: the process that ran the edge, on success, moves the Task to `to`
  if it is still on that edge;
- set: put the Task at a named stage.

**3. Database ↔ CLI.** The CLI calls the store, never SQL.
- Read: `lf task status ISSUE [--json]` carries graph, position, the ways out
  of the current stage and history.
- Write: `lf task run ISSUE [CHOICE] [--reason NOTE]` chooses and runs, or
  takes up a workflow by name; one new command sets a stage. Who is taken
  from the calling conversation when there is one, else the person.

**4. Store → stream → SwiftUI (aligned with LOO-382, PR #1452).** Jack,
October 6: "Look at the work in 382. Figure out what their vision of the
future is and align with it." That design: Desktop is reactive to the store
and nothing else. SQLite triggers bump a revision per domain (`planning`,
`sessions`, `flows`, `execs`, `usage`) on every write; one `lf monitor
workspace --watch --json` per window streams parts whose bodies are the
existing `--json` DTOs; after its own write Desktop sends `refresh` and waits
for the frame that answers it. Forbidden there: polling beside the stream, a
second cache, Desktop-only shapes, a read that writes an Exec, revision bumps
at call sites. For the Workflow:
- Position and moves are stored rows, so a change is a store write the
  triggers see. Nothing Desktop shows is computed only at read time, except
  liveness, which stays unknown until observed.
- Every table this PR adds gets a domain: Workflow tables `planning`;
  `flow_execs` and `flow_exec_steps` `flows`; `session_activity` `sessions`,
  bumping only when Waiting could change, with quiet-time Waiting on the
  watch's clock.
- Desktop reads the Workflow and the Flow exec log from the `task` part
  (`TaskWork`), not `lf task status` or `lf flow show` per view; its buttons
  run `lf task run` or the set command, then `refresh`.
- #1452 merged first (`c787c7530`, October 6); its triggers on
  `flow_sessions`, `flow_events` and dropped `agent_sessions` columns are
  reconciled in this PR's one draft. The rest of this layer is built as
  slice 12; the Flow exec log rides in the `task` part beside `TaskWork`.

Jack, October 6, on `lf task run` taking the only way out, erroring at a
stage with several until one is named, erroring on a name that is not a way
out, and running ad hoc for a Task with no workflow: "that talbe looks
right". Choosing and running stay one command.

Built as slice 9: a stopped edge holds the Task on the edge. Still open:
whether a finishing edge should reach the conversation beyond its own
background tool or reading status (default: no).

## Built

Slice 1's account of what was built is at `74a738f72:scratch/task-workflow.md`;
its unreviewed choices are in [questions.md](questions.md).
