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
4. **Where.** Default: PR #1439, after FlowExec.
5. **Names.** Default: builtin workflows keep `feature`, `code`, `research`;
   the flows they run are `task-design`, `pursue`, `ship`. `lf feature` then
   names the workflow's `start` edge through `lf task run`; a bare `lf run
   feature` outside a Task is an error that says so.

## Build order for the pass

1. Loader and validator for `.lf/workflows/*.yaml`: stages, edges, implicit
   `start`/`end`, every `flow` resolves to an operational Flow, unreachable
   stages rejected. Builtins `feature`, `code`, `research`.
2. The record and its migration, in the existing draft: one TaskWorkflow per
   Task (name, captured graph) and a traversal table (edge, FlowExec, time).
3. `lf task run ISSUE [FLOW]`: start the TaskWorkflow on first use from the
   Project's default; resolve the edge; append the traversal; enter ordinary
   `lf run`. Refuse a flow that is not an outgoing edge of the current stage,
   naming the edges that are. Plain `lf run` stays ad hoc.
4. `task status --json` and the Desktop DTO carry the workflow graph, the
   position and the running edge's FlowExec. Text status prints one line.
5. Stage guidance: the Task conversation's instructions name the current
   stage's skill and the outgoing edges; builtin skills stop describing human
   steps inside flows.
6. Tests: a 0-PR workflow reaches `end` with no PR; a loop through a landing
   edge twice; a failed edge returns to its `from` stage; a plain `lf run`
   leaves position unchanged; a Task with no TaskWorkflow runs flows as
   before.
