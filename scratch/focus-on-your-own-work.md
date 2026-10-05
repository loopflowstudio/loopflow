# Task conversation and ordinary Flows — design and state

October 5, 2026. LOO-353, Product, PR #1439. Written after Jack Heart's
delivery review of `e10e2add4`; the pass he then requested is built. The
reviewed evidence and Jack's exact words are in
[demo-task-flows.md](demo-task-flows.md); open choices are in
[questions.md](questions.md); the code walkthrough of the reviewed revision
(before this pass) is [pr-review.html](pr-review.html). The pass's step-by-step
plan is at `67cd68157:scratch/focus-on-your-own-work.md`; the FlowSession
architecture it removed is at `6f246fda4:scratch/focus-on-your-own-work.md`.

## Flow model — decided October 5, governs the rest of this document

**Not settled; do not implement from this section yet.** After it was written
Jack said of the outer workflow (start node, land node, human sessions
between, edges are `lf` flows): "I think maybe the flowsession is *that*". On
that reading FlowSession is the mutable, takeover-able outer run, and one
`lf` flow run is a process with a driver-kept, append-only record (Jack
earlier: "something more similar to an Exec but specifically for Flows"). The
text below assumed FlowSession was the record of one `lf` flow run. Awaiting
Jack's confirmation; the rules on uniform tracking, driver-maintained state
and oblivious steps hold under both readings.

Jack Heart's statements are quoted in [questions.md](questions.md) under
"Step invocation". Where any section below still describes a Flow as only its
driver Exec and step Execs, or steps carrying a `FlowStep` payload, this
section replaces it.

Three records, as for conversations:

- **Exec:** one `lf` process. Immutable fact.
- **AgentSession:** one conversation.
- **FlowSession:** one run of a Flow. It has its own id and is mutable. Jack:
  "maybe we still want FlowSession. And then the FlowSession is mutable, but
  the Flow exec is not." It is not 1:1 with a process, "in the same way that a
  simple interactive skill session is not 1:1 with its launching process".

FlowSession holds the Flow's name, the graph as compiled at launch, the
current position, a step log (each step's child Exec, graph key, iteration and
result) and the Flow's own outcome (running, completed, blocked, stopped,
handed off). Driver Execs link to it; none is its identity.

Rules:

- **Uniform.** Every Flow run has one, ad hoc or started by `lf task run`.
  None is primary for a Task; no Task column points at one. Membership is the
  driver's directory. A primary Flow may return later as a layer on top.
- **The driver maintains it.** Queryable, scalable, performant state is the
  Flow process's job. Clients (`lf flow list/show`, `task status`, Desktop)
  read it and work for any Flow.
- **Read-only to everyone else for now.** No API changes a running Flow; the
  control is ending its driver. A record that says running while its driver
  is dead is presented as stopped, never stored as a contradiction.
- **Steps are oblivious.** The driver starts each step as the plain command, a
  child process: `lf -b skill <name> [message]` or the operation's own
  command. No `FlowStep` payload, `--__flow-step`, `__flow-step` command or
  position variable. A step never reads or writes the FlowSession. A deciding
  or routing step gets its answer contract in its message; the driver
  validates it and corrects it by resuming the same conversation.
- **Gone for good:** worker claims and generations, automatic recovery, the
  pending-review pointer, the `replaced` state, steps reading a cursor row.

Designed for, not built in this pass: Jack, "in the same way the session api
lets you replace / take over any lf skill, the flow session api would let you
replace / take over any running flow". The record therefore keeps enough
position for another driver to continue, and the driver link is replaceable.
Whether takeover is built in this PR is unanswered.

Implementation: reshape the FlowSession code that existed before `68c1ffc55`
(`store/sqlite/flows.rs`, `store/flows.rs`, the DTOs and Swift decoders at
`88942accf`) rather than writing a new record beside the Exec-derived readers.
The one draft migration reshapes `flow_sessions` and `flow_events` instead of
dropping them: remove claim, generation, pending-session and review columns
and the Task pointer; keep every row as history.

## Outcome

One native Task conversation holds design and review, files, drafts, shells and
layout. Headless work is the ordinary `lf -b` command run in the Task's
worktree. Running a Flow leaves nothing behind beyond what `lf` logs for any
command.

## Decisions by Jack Heart

October 4 (unchanged):

- Interactive Task design and review stay in the ongoing Task conversation;
  more conversations may be opened deliberately. Headless work goes through
  ordinary `lf -b`; substantial implementation in the conversation is
  discouraged, incidental edits allowed.
- Product says **Session** for interactive and **Run** for headless
  AgentSessions; identity, history and attribution survive mode changes.
- **Waiting** is the one attention state; `--waiting` replaces `--needs-me`;
  `--interactive` stays a mode filter. Provider-specific signals and false
  positives are acceptable. Claude: binary stream-json only.
- Delete all Task-worker machinery, mutable Flow switching and automatic
  database-backed recovery. The caller owns recovery.
- Delete `lf session ready`/`complete` and the review handshake; no renamed
  replacement. Conversational feedback chooses what runs next.
- Task membership is owning Home plus resolved checkout plus explicit binds,
  excluding repository/Wave scopes. Hiding or selecting never terminates or
  approves.
- Approved design: conversation-stage workflows with operational-Flow edges;
  operational Flows hold autonomous loops and XORs only.

October 5 (delivery review):

- The cut "should have been DELETING the task worker APIs and routing more
  things through the basic (e.g. flow -b) apis." Task status presentation is
  "not the current focus of the task."
- "-b should continue to print the output and block just like it used to."
  `--mode`: "Definitely take this back."
- "running somethign from a task's worktree and passing --task should be
  equivalent ... we shouldnt get different codepaths or validations for
  either."
- "`lf task run` or whatever is allowed to do #1 and #3 and then go into the
  lf run flow": place the Task; fill defaults. On launch-time PR preparation
  (#2): "im not sure why we need 2."
- "there shouldnt be anything extra on top of what you get when you run
  `lf flow ...` in a taks worktree. we should still have good records of whats
  flows are running and at what stages just via normal logging from lf binary.
  I dont think we need the FlowSession datatype." Then: "fold it into this pr."
- `lf commit`: "should always push i guess ... or else should accept -p."

## Branch state after the October 5 pass

Built on `67cd68157`, merged with main at v0.13.3 (`8ea0bec9c`, release files
only, no conflicts); Jack Heart has not reviewed it.

- **Task entry.** `lf task run ISSUE [FLOW]` places the Task (worktree,
  `--stack-on`, checkout restore), defaults to the Project's Flow, records `-m`
  as the Task's agent and `--reason` as a steer, then continues as
  `lf --task ISSUE run FLOW`. It honors `-b`/`-i`, prints and blocks. Gone:
  `flow start`, the tmux launcher, `LF_TASK_FLOW_OPTIONS`, launch-time PR
  preparation and agent preflight, `task create --run/--flow`.
- **One path.** Every Flow launch that resolves to a Task, by the entry,
  `--task` or its worktree, gets the same check: ready Work, matching
  planning, current chapter, not being abandoned.
- **No FlowSession.** A Flow is its driver Exec and the step Execs it starts.
  The driver holds graph and cursor in memory and tells each step what it is on
  argv (`--__flow-step '<json>'`: Flow name, launch number, label, cursor,
  graph key, per-edge iterations, required answer, Session to continue).
  A step's result is its exit; a deciding or routing step's answer is the final
  answer of the Session turn its Exec captured. An invalid answer is corrected
  in the same conversation, three turns at most. Gone: `flow_sessions`,
  `flow_events`, `agent_sessions.flow_session_id`, the cursor row, position
  versions, attempts, turn selection, the driver lock, `LF_FLOW_STEP`.
- **Readers.** `task status`, completion/cleanup/landing blockers, chapter
  "started", `lf flow list/show --sessions`, `lf monitor`, Session Flow
  membership and Desktop's Task work read Execs. A Flow is `current` (driver
  has no recorded exit), `completed` or `stopped`; a failed Flow no longer
  reads as current. A past Flow whose YAML changed is drawn from its step
  sequence.
- **Migration.** The one draft archives each Flow row's name, state and last
  cursor as a `legacy_flow` observation on the Sessions it opened, keeps Task
  Started evidence, drops both tables and the Session column, and rewrites the
  ancestry and Started triggers.
- **Callers.** Builtin skills, `skills/loopflow/SKILL.md`, docs and README name
  `lf task run` and say it blocks; Desktop Start owns the command as a child
  process and reports an immediate refusal.

## Remaining from this pass

- **Unproven.** A real provider step; a driver killed mid-agent-step (the
  surviving provider turn is left to its Session, nothing settles it into the
  Flow); the migration against a populated store; Desktop Start against a real
  `lf`. The real-Codex e2e (`tests/e2e/codex_connect.py --flow-decision-retry`)
  is ported but not run.
- **Publish after an unobserved merge.** The proof covers a merge already
  observed: publication refuses, names the merged PR and `lf pr next`. A merge
  nobody has observed still reaches GitHub through `pr publish`; `lf pr
  reconcile` remains its owner.
- **History not archived.** A saved Flow that opened no Session (operations
  only) leaves no observation; its Execs remain.
- **Desktop.** Task work now decodes `execution.work`, where `lf task status
  --json` has emitted it since #1379; before, the read failed on real output.
  Desktop does not draw step Execs or why a Flow stopped.
- **Leftovers.** `SessionKind::FlowReview` and `SessionAttention::Review` are
  unreachable for new work and go with the Waiting slice. Task status still
  lists every Exec (defect 2 in [the demo notes](demo-task-flows.md)).
  `--needs-me` remains in the CLI, docs and `repo/operate` until `--waiting`
  replaces it.

## Later slices (approved October 4, unbuilt)

Flow history in these means Exec sequence.

- **Two notions of Flow.** Jack (October 5): "two different notions of flow.
  One where there there is a start and a land node and then in between are
  human sessions, and the edges are lf flows". The outer one is the workflow
  below; each edge is an operational Flow with a FlowSession. A workflow has
  no record of its own yet; the Task-level "primary" layer Jack deferred
  would sit here.
- **Workflows.** `.lf/workflows/<name>.yaml`: conversation-stage keys with
  optional review skills, and edges with `from`, `to`, `flow`. Source guidance
  only: no Work kind, runtime row, cursor or token, never compiled into a Flow
  graph. The Task conversation reads results and launches the next edge with
  `lf -b`. Questions, holds and ambiguous replies launch nothing. Author the
  builtin kickoff/review and demo/revision stages this way.
- **Loops.** One form: a deciding node last, `loop: <target>` with optional
  `step` defaulting to `loop-or-next`, replacing `loop-decide` and
  `repeat.from` in catalog, skills, templates, docs and tests with no alias.
  Unique skill names resolve the target; one optional occurrence name
  disambiguates. Advance, Iterate or Blocked; malformed output never advances.
  Shared and overlapping return edges are preserved.
- **Task primary.** `PrimaryScope::Task(TaskId)` under the existing scope
  lock. Explicit choice wins; else the sole unfinished interactive Task
  conversation; else the most recent by `latest_interactive_session`'s ranking
  (native human input, interactive opening, creation), restricted to
  unfinished Task members. Read-only inventory creates nothing.
- **Waiting.** Rust-owned. Immediate on explicit pending input or a successful
  interactive yield with no outstanding tools; otherwise after 120 seconds
  without provider activity and zero unresolved tool calls. No event yet is
  opening/unknown; disconnection is unknown. New activity clears it.
  `session list --waiting` filters before paging. Evidence per provider:
  [harness-attention.md](harness-attention.md).
- **Desktop.** Every Flow of the Task gets the same graph/progress/output
  view; Waiting first, working Sessions in a compact group; no completion
  controls.
- **Workspace proof and scope retained from September 30:** real provider
  continuation, owning-Home remote association, cross-Task focus/input and
  process retention, same-byte symlink/read-only transitions (private Home
  copy only); twenty comparable retained-layout actions against p95 <100 ms
  with idle CPU/process counts; Project Default Flow in Wave settings; Edit
  Flow opening real source with explicit builtin customization; website
  alignment. Details: [workspace review](../docs/reviews/task-workspace.md).

Exclusions: general messaging, automatic recovery, a Run entity, Project
reset (LOO-366), completion-policy redesign (LOO-367), Claude hooks/SDK,
external-progress credit from self-hosting, Task status presentation.

## Checks

Clear inherited `LF_*`/`LOOPFLOW_*` before Rust tests. Gate owns the full run.

| Command | Proves |
| --- | --- |
| `cargo test -p loopflow --test task_flow_launch_tests` | The entry, `--task` and a worktree launch run in the foreground, leave identical Execs and get identical refusals; the default Flow; removed commands. |
| `cargo test -p loopflow --test session_lifecycle_tests` | Against a fake provider: decisions, same-conversation correction bounded at three turns, routing, three loop passes under one driver, a blocked decision. |
| `cargo test -p loopflow --test flow_tests --test flow_discovery_tests` | Flows read back from Execs; a Flow whose definition is gone. |
| `cargo test -p loopflow --test pr_tests publishing_after` | Publish after an observed merge names the PR and `lf pr next`. |
| `cargo test -p loopflow --lib store::migrations` | Released frontier converts through the one draft. |
| `cargo test -p loopflow --test dto_fixtures`, `swift build --build-tests` | Wire shapes agree across Rust and Swift. |
| `git grep -E "FlowSession\|flow_sessions\|flow_events\|flow start\|LF_TASK_FLOW_OPTIONS"` | Outside scratch and Wave memory: migrations and their tests, two dated reviews under `docs/reviews/`, and old release notes. No executable path. |

Last run, October 5 after the merge: `cargo test -p loopflow --lib
store::migrations`, 83 passed; the rest wait for gate.

Configured demo, still separate: in a private Home, `lf -b task run INF-123
proof` prints and blocks; `lf monitor` and `lf flow show ID --sessions` show a
looping Flow's driver, step and iteration; a real provider step; Desktop.
