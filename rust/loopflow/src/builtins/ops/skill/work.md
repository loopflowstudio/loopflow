---
description: Use Loopflow's Task, Wave and Project APIs while preserving the caller's workflow.
---
# Working with Tasks and Waves

Use this contract when the request uses work APIs or continues an existing
Task. Desktop and custom clients use the same APIs. Work association supplies
context and ownership; the request, selected Skill or authored Graph supplies
the procedure. Reading this skill creates nothing and starts no work.

## Work model

- **Wave**: a durable objective, memory and area of responsibility.
- **Project**: a Wave's plan, including Tasks, KRs and targets.
- **Task**: one repository-owned intended outcome with optional Wave membership, its checkout, conversations and a current PR and delivery history.
- **Graph**: authored steps, some `interactive:` where a person takes part. A
  Task takes up one and stands at a step of it; a run executes it, with or
  without a Task.
- **Session**: a conversation. **Process**: one actual lf process.

Continue the selected Task or the current checkout's existing Task. Preserve
its identity, checkout, PR and live work. Unrelated Tasks in the repository do
not select work for this conversation. Do not create a Task merely because a
worktree, PR or Graph exists. Create Tasks when requested or when the selected
planning procedure calls for them; taskless work remains taskless otherwise.

Use `lf` for changes to managed worktrees, Task delivery and Work state so their
recorded ownership stays consistent. Ordinary inspection, edits and tests run
here. Preserve existing contributions before checkpointing with `lf commit`.

## Inspect and act

Read `lf task status <issue> --json`, `lf wave status <wave> --json`,
`lf wave list --json`, or `lf wave show --json` for the requested scope.
Use these shared reads instead of reconstructing Work state from processes or
worktrees. Unknown or unavailable evidence is not an empty plan. Report the
missing service and continue independent work; do not repair auth unasked.

Before starting more execution, inspect the Task's live work, previous results
and effects. Leave a live Graph running. A stopped Graph is history: repair the
cause and run the steps it did not reach, without replaying effects.

`lf task run` takes up the Task's graph when it has none and runs it to the next
interactive step; `--from <step>` starts elsewhere.
`lf task move --wave/--no-wave` changes membership and preserves existing work. Review
feedback, Task association, and an idle process are not instructions to launch.
A finished Graph or published PR does not prove Task completion. Complete the
Task when its accepted outcome is delivered, using authoritative state.

For ongoing supervision, explicitly select `task-operate`, `wave-operate` or
`repo-operate`, or the corresponding Session skill. Operators move your
existing Tasks and leave other people's Tasks and backlog alone. A contribution
to a Task need not operate it, choose a graph, or keep other Tasks moving.
Custom clients retain their own interaction and workflow choices.

## Delivery and durable context

Use the selected delivery skill for publishing, reviewer handoff or landing.
Publication, merge and Task completion are distinct. Keep additional accepted
outcomes in separate Tasks when useful, with links. Continued publication in the
same Task selects a new current PR and preserves earlier delivery history. No Task is
required for ordinary PR delivery.

Read the supplied brief, accepted design and relevant Wave memory. Keep working
plans in scratch. Wave context is read-only unless the selected Loopflow Graph
calls for maintaining it. Task/Wave association and use of Work APIs alone do
not authorize writes to `.lf/wave/`. Keep proposed memory updates in local notes
outside that procedure; never start a Graph merely to obtain write authority.
Never guess a Wave merely to complete a template.

Use scratch freely for plans and notes; its Markdown enters context automatically.
`lf pr submit` and `lf pr land` invoke `lf scratch clear` during preparation.
It removes added files from the work's diff and retains existing files and edits.
The command is also available on its own. PR open and publish do not run general scratch cleanup. Do not clear
the directory manually.

Keep routine progress in this conversation and local notes. Task descriptions
hold the current problem, outcome and constraints; deliberate new direction
belongs in Task comments. Preserve `<!-- loopflow-progress:... -->` provenance
for published agent progress; `lf comment --steer` is only for new direction.
Selected operators read failure logs and discuss missing judgment in their
ongoing conversation. An open conversation does not schedule another turn;
Graphs and installed schedules run independently.
