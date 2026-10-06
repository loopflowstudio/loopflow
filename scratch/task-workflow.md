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

## Proposed shape

**Authored.** `.lf/workflows/<name>.yaml`. Stages are where a person takes
part in the Task conversation; edges are `lf` flows. `start` and `end` are
implicit.

```yaml
# feature
stages:
  design: review-design      # skill the conversation uses at this stage
  demo: demo
edges:
  - { from: start,  to: design, flow: task-design }
  - { from: design, to: design, flow: task-design }   # revise the design
  - { from: design, to: demo,   flow: pursue }
  - { from: demo,   to: demo,   flow: pursue }        # another pass
  - { from: demo,   to: end,    flow: ship }          # land
```

A 0-PR Task is a workflow with no landing edge (`research`: start, findings,
end; the last edge may name no flow). Several PRs are a loop through a
landing edge back to a stage. The workflow never counts PRs.

**Record.** One TaskWorkflow per Task that has one; a Task may have none and
run only ad hoc flows. It holds the Task, the workflow name and graph as
captured when it started, and a traversal log: edge, the FlowExec that ran
it, who started it. Position is read from the log, not stored: on an edge
while that FlowExec's driver runs; at the edge's `to` stage when it finished;
back at `from` when it stopped or failed.

**Moving it.** One helper, already allowed to do extra work before entering
ordinary `lf run`: `lf task run ISSUE [FLOW]`. With a TaskWorkflow it
resolves the outgoing edge (the only one, or the one whose flow is named),
appends the traversal, then runs the flow. Starting a Task runs the `start`
edge. There is no approve, complete or advance command: a person's feedback
in the conversation leads the conversation to run the next edge. A plain
`lf run` in the worktree is tracked as a FlowExec and does not traverse.

**Reading it.** `task status` and Desktop draw the workflow with the current
stage or edge, and the running edge's FlowExec inside it. "At a stage" means
the Task waits on a person. This replaces LOO-317's derived view.

**Builtins.** `feature` (design, demo, land), `code` (demo, land), `research`
(no PR). The Project's default names a workflow.

## Open, for Jack — the pass follows the default

1. **Position.** Default: read from the traversal log and moved only by
   `lf task run`; never set directly.
2. **Replace and take over.** Default: replace switches the Task to another
   workflow, keeping the log; no take-over command is built. Jack has not
   said which of these he means: switching workflow, another conversation
   becoming the one that moves it, or restarting a running edge.
3. **Reaching `end`.** Default: it does not complete the Task; `lf task
   complete` stays separate (LOO-367).
4. **Where.** Unresolved; Jack said both. "I think you own TaskWorkflow. We
   cant do this design correctly without it" places it in LOO-353; asked
   whether to keep this PR to the inner record and leave the outer one as the
   next Task, he said "Yeah i think so." The FlowExec pass built none of it.
5. **Names.** Default: builtin workflows keep `feature`, `code`, `research`;
   the flows they run are `task-design`, `pursue`, `ship`. `lf feature` then
   names the workflow's `start` edge through `lf task run`; a bare `lf run
   feature` outside a Task is an error that says so.

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

Not built: a take-over command (open choice 2); Desktop drawing (slice 6);
workflow defaults in Wave settings and source editing (slice 7); docs
(slice 8).
