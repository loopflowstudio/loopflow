# Open assumptions

LOO-298 · 2026-09-29 · Jack Heart's decisions are in
[the contract](data-model-one-table-per.md). Earlier questions and superseded
proposals remain in [committed history](https://github.com/loopflowstudio/loopflow/blob/d6dc8c43b73872b8b6e71b0996468d2432b23102/scratch/questions.md).

- **Usage after bind:** prospective attribution remains the operating assumption.
  Jack leans toward post-hoc attribution but has not selected it. Earlier events
  keep their recorded owner; an active turn's cumulative usage cannot silently
  move or be split without evidence. Bring this choice to concept review.
- **Blocked feedback:** retain `lf flow blocked` as a keyed Ask returning feedback
  under the selected conversation's current driver. It supplies no verdict.
  Jack has not selected a replacement; bring the retained interaction to review.
- **Router removal:** remove it with the decision command as the operator's
  assumption under Jack's autonomous goal. Structured output is the only verdict
  and route writer. No compatibility command remains.
- **Captured events:** use kind `captured`; reservation inserts it before launch.
  Preserve artifact directory keys and receipt strings as immutable evidence,
  with sequence identity for current selection and publication. Unclassified SQL
  goes to immutable `import_evidence`, never a fabricated AgentSession. This is
  an import archive with no runtime reservation or lifecycle writer.
- **Runtime passes: decided, not built.** Jack Heart removed child pass
  FlowSessions on 2026-09-30 (entry below). A pass is a node and iteration
  position. The earlier child-pass interpretation and its proofs are superseded.
- **Whole-Flow binding:** outside Session bind scope. A taskless Flow and its
  member Sessions cannot acquire incompatible Tasks piecemeal.
- **Wave without a current Project:** automatic creation on Task start remains
  unselected. Report the missing current plan.
- **Exec discovery:** list defaults to 100 rows in the current repository;
  `--all` spans repositories and `--after` uses the prior JSON cursor with the
  same filters. Pagination is not snapshot isolation. Every-process observation
  still needs final disposition for parser/help/version, bootstrap/installation
  and screenshot paths; missing completion is never a successful receipt.
- **Import and proof limits:** original SQL payloads, selectors, ancestry and
  Started remain evidence even without known conversation membership. Imported
  observations cannot settle a native Flow turn. Local retirement cannot prove
  absence of another Home's work; fixtures cannot establish configured accounts,
  rendered Desktop or installed migration. Retain original artifacts until the
  full import obligations pass. Prepare the real-Home procedure without executing
  a branch binary against the installed Home or promoting it.

No additional product decision is needed to continue implementation. The
[remaining work](remaining-work.md) and [import obligations](import-preservation.md)
retain acceptance and preservation requirements.

- **Discovery surface:** use `flow list/show --sessions` for saved FlowSessions,
  keeping the proposal's `sessions` and `inspect` names available as templates.
  Session inventory has an explicit page mode for Desktop; ordinary alphabetical
  inspection remains. These are implementation choices, not attributed decisions.

2026-09-29 · **A launch is an Exec: decided, and part of LOO-298.** Jack Heart
asked "why not have lf flows actually launch skill execs?", said it "makes the
hierarchy way better and clearer", and then directed: "this is part of 298" and
"rearrange the tasks around this -- it's central and we should do the deepest
cuts first." Each Flow step runs as its own `lf` process. The Flow driver keeps
the claim and spawns one child `lf` per step; mechanical steps that are already
lf commands run as child Execs too. No new object: a step is an ordinary Exec.
The operator had filed LOO-335 for this minutes earlier, before Jack's
direction; it is superseded by LOO-298 and awaits Jack's word on deletion.
Implementation choice in [the Exec design](exec-per-step.md): process start/exit
belong only to Exec. Retain captured instructions as a Session history event,
not a second launch lifecycle; capture belongs to the executing child when a
process is known. Imported missing Exec stays unknown. Jack has not reviewed
this implementation choice yet.

2026-09-29 · **Vocabulary: prefer Exec.** Jack Heart: "if a launch is an exec,
we should also try to use the word exec instead of launch (or run etc) where
possible." This supersedes the operator's suggested word "launch" and the
assumed event kind `captured`/`launched`. Names, CLI text, docs and wire fields
that mean one agent start under one lf process say Exec. Where no lf process
exists, as with imported history, the name must not claim one.

2026-09-29 · **After Exec-per-step: simplify attribution and the parent tree.**
Jack Heart: "Make sure that we simplify and clarify attribution and the parent
tree after this." This is a required follow-on inside LOO-298, directly after
the Exec-per-step cut. It is a reduction: name the one place each fact lives,
derive the rest, and delete the copies.

2026-09-29 · **Requirement kept through the simplification.** Jack Heart: "to be
clear i still want being called by an agent process to give you the right
parent-lf process as your lf exec parent." An lf command issued by an agent
records, as its parent Exec, the lf process that is driving that agent. The
provider and shell processes in between are not Execs and are skipped.

## Mechanical child entry · 2026-09-29

Implementation choice under Jack Heart's accepted Exec-per-step direction:
`__flow-step ID VERSION` is a hidden entry for captured boundaries. Internal
children use the invoking driver's executable and inherited store, so a PATH or
selected-installation change cannot silently swap schemas during a Flow.
**Tension, open for Jack:** this keeps a Flow on one binary for its whole run,
while LOO-334's accepted direction is that each step picks up the official
installed lf. Safe schema handling across steps is what reconciles them. The
existing full Task claim travels as launch context and remains owned by the
driver; no second durable authority is added. Agent conversion is still pending.

2026-09-29 · **Close the leak fully.** Jack Heart: "The thing we're fixing where
lf flows invoked skills without an exec is a big leak to me and suggests we
likely reimplemented some skill machinery inside flow. Let's make sure that gets
fully closed." Finish line for item 1 is therefore stronger than moving the
provider spawn: a Flow step runs a skill only by executing the same `lf` skill
command a person would run, and no skill machinery remains inside Flow or the
Task controller. Anything a Flow needs beyond that is passed to the command as
input, not reimplemented beside it.

2026-09-29 · **Step kinds.** Jack Heart, correcting the operator's skill-only
framing: "Flows can have ops OR skills OR subflows." The Exec-per-step finish
line covers every step kind, not only skills. Open, for Jack: whether a subflow
step runs as its own `lf flow` child with its own FlowSession, or stays expanded
into the parent's captured graph as the accepted contract currently says
("template composition expands the graph; only runtime loop nesting creates
parents").

2026-09-29 · **Subflows: decided.** Jack Heart: "Flow invocations should be fully
rolled out when they are run actually, so it's all skills and ops." A subflow is
an authoring construct. At capture it is expanded into the one FlowSession's
graph; at run time every executed step is a skill or an op, each its own Exec.
No nested driver, no FlowSession per subflow. This confirms the accepted
contract and closes the open question above. Runtime loop passes still create
child FlowSessions.

2026-09-29 · **Vocabulary: compile.** Jack Heart: "the flow definitions /
templates can have flows, but those get 'compiled'." A Flow definition may
reference other Flows. Starting it compiles the definition into the
FlowSession's graph of skills and ops. Use "compile" for that act in names and
docs, in place of expand, flatten, concretize or roll out.

2026-09-29 · **Subflow is a lens.** Jack Heart: "i still think you could
coherently talk about a subflow but it would be more of a lens than an
operational entity." Each compiled step keeps which definition it came from, so
a view can group steps by subflow. A subflow has no FlowSession, no Exec, no
claim and no lifecycle. The label is display provenance and grants nothing.

2026-09-29 · **"Parent" is the wrong word twice.** Jack Heart, on the loop parent
and the template provenance: "Neither parent definition looks like what i would
think of really but worth coming up with different language." Reserve "parent"
for the one thing that is a parent: an Exec's parent Exec in the process tree.
Operator candidates, not Jack's choices: a loop pass names the Flow it is a
`pass_of`; a compiled step names the definition it is `from`.

2026-09-29 · **Direct behavior wins.** Jack Heart, on the differences between
running a skill directly and running it as a Task step: "Run directly seems like
it wins there always." Decided for the seven differences shown to him: agent
precedence, transient retry and failover, raw provider recording, journal skill
events, the Git-operation fence, context flags, and the printed context summary.
Operator assumption for the four he was not shown in that table: they are Task
input passed to the same command, not a second behavior. Those are the Task seed
with live steers, the user name, the worktree write scope with its execution
boundary, and capability blocker detection.

2026-09-30 · **Loop passes are not FlowSessions: decided.** Jack Heart asked
what child FlowSessions per loop pass were for, was shown that a pass is already
identified by its node and iteration tuple, and said: "yeah, lets remove them."
One started Flow is one FlowSession. A loop pass is a position in it and a lens
over its history, like a subflow. This reverses the earlier contract line "only
runtime loop nesting creates parents" and the runtime-children slice built on
2026-09-29.

2026-09-30 · **Which lf a child runs: decided.** Jack Heart: "i like the default
behavior being pick up new lf, but i do think it should be possible to lock an
lf binary so that all children get that lf, but it should be sort of recursive
so that we somehow manipulate the path so that the locked lf is what you get
when you use `lf` within there." Default: every child lf, including Flow steps,
resolves the currently installed lf. Lock: an explicit choice that puts a
directory holding the locked binary as `lf` first on PATH, so every descendant,
including commands an agent types, gets the locked lf. This resolves the tension
recorded above in favor of the default. The lock mechanism belongs to LOO-334;
LOO-298's step entry must stop using the driver's own executable and resolve
`lf` the same way an agent's shell does.

2026-09-30 · **Jack Heart's answers to the six open items.**
- **Usage after bind: prospective, decided for now.** "prospective is ok for
  now, maybe follow-up task in intelligence to re-evaluate." A follow-up Task
  in the intelligence Wave re-evaluates it.
- **Blocked is a decision value: decided.** "i think blocked should be one of
  the possible values of the decision structured type? not sure where else it
  would make sense." Remove `lf flow blocked`; a decision step returns
  advance, iterate or blocked, with its reason. The person's answer starts the
  next turn.
- **Template provenance: likely unneeded.** "dont know that we even really need
  this to exist." Delete the compiled step's source-definition field unless a
  real consumer needs it; report any that does. The subflow lens stays a
  description, not a stored field.
- **The four Task-input assumptions: held.** "hold this to discuss again." Not
  decided; do not treat the operator's assumptions as accepted.
- **LOO-335: deleted** at Jack's direction.

2026-09-30 · **Task context follows the checkout: decided.** On the first of the
four held Task-step behaviors, Jack Heart: "Yeah the task context should be
automatically passed in when you do any skill (or flow) inside a task worktree.
shouldnt need to explicitly use task commands." Any `lf` skill or Flow run in a
Task's worktree receives the same Task context a Task step gets, including live
steers and workspace details. No `--as task:X` or Task command is required.
The other three held behaviors remain open.

2026-09-30 · **One user name for every lf run: decided.** Jack Heart, on the
second held behavior: "what my name is should be common to all lf runs
ideally." Every agent start, whatever launched it, renders the same user name
from the one resolver. No launch path omits it or supplies its own.

2026-09-30 · **Worktree confinement: unchanged.** Jack Heart, on the third held
behavior: "sounds like this doesnt actually change?" Correct: a Task Flow step
keeps worktree confinement, the execution boundary and skipped prompts.
Open follow-on for Jack: Task context now follows the worktree, but confinement
is still attached only to Task Flow steps. A run a person starts in a Task
worktree gets the context and their configured permissions. Either keep that
split (confine unattended runs) or confine anything run in a Task worktree.

2026-09-30 · **Credential failures belong to accounts, not Tasks: decided.**
Jack Heart, on the fourth held behavior: "Yeah dont think tasks should be
involved in this. good example of where responsibility is not distributed to
the right places." The Task-only "capability blocked" detector stays deleted.
The shared launch path fails over, and the account layer records the revoked
credential on the account (`record_credential_invalidated_blocking`, which sets
`credential_state` to missing), where `lf auth status` and the route show it.
All four held Task-step behaviors are now settled; one follow-on on
confinement remains open.

2026-09-30 · **Task commands are shortcuts: decided.** Jack Heart: "the key thing
is that the lf task run commands are just shortcuts but dont imply any different
behavior than just running the equivalent lf flow directly." `lf task run X` is
`lf flow <the Task's Flow>` run in X's worktree, and nothing else. Any behavior
a Task step has must come from something the equivalent `lf flow` invocation
also has: the checkout, the Flow, headless or interactive mode, or explicit
flags. Consequence for confinement: it can no longer key on "launched as a
Task". It keys on the worktree or on unattended execution; either satisfies
the rule. Operator's reading; Jack has not picked between them.

2026-09-30 · **What identifies the Task: decided.** Jack Heart: "and --as and the
worktree are what identify the task, not having a parent exec that is lf task
..." A run's Task comes from `--as task:X` or from the checkout it runs in.
Never from process ancestry, a parent `lf task` Exec, or inherited environment.
Candidates this removes: the calling Session's Task inherited by agent-issued
commands (`agent_work_in`, WorkSource `inherited`), `LF_TASK_ORIGIN` carried to
descendants, and Task identity read from `LF_WORK_ADVANCE_CLAIM`. The claim may
still fence the driver; it does not name the Task for a step.

## Confinement implementation boundary · 2026-09-30

Following Jack Heart's request for one predicate, the current slice keeps the
intersection of unattended execution and a Task checkout, now shared by direct
skills and ordinary/managed Flows. This is an interim preservation choice, not
approval of either wider policy. The two options remain checkout-only (including
interactive commands) or unattended-only (including unbound work). See
[the branch audit](task-command-equivalence.md).

2026-09-30 · **An ancestor's --as is inherited: decided.** Jack Heart, refining
the rule above: "Well, a parent --as should also work. i guess that could use an
env variable after all." A run's Task comes from `--as` on this command, `--as`
declared by an ancestor and carried through one environment variable, or the
checkout. What is inherited is the explicit declaration, not ancestry: being
under `lf task`, holding a claim, or being called by an agent Session still
names no Task. Operator assumption, open for Jack: precedence is this command's
`--as`, then an inherited `--as`, then the worktree.

2026-09-30 · **Task commands set --as.** Jack Heart: "using lf task should
automatically use --as for subcommands basically." `lf task run X` is exactly
`lf --as task:X flow <the Task's Flow>` in X's worktree. Its children see the
Task through the ordinary inherited `--as`, not through a Task-specific path.

2026-09-30 · **Task precedence: decided.** Jack Heart: "i think worktree beats
inherited as." Order: `--as` on this command (including one implied by an
`lf task` command), then the checkout's Task, then an `--as` inherited from an
ancestor. This replaces the operator's earlier assumption, which put inherited
`--as` ahead of the worktree. Causation stays in the Exec parent tree.

## Explicit Work declaration refinement · 2026-09-30

Jack Heart decided that explicit `--as` on this command wins, followed by the
Task checkout, followed by an ancestor's explicit declaration. The earlier
operator assumption putting inheritance before checkout is superseded.
`lf task run X` is the explicit `--as task:X` shortcut. Use one variable (`LF_AS`;
no existing declaration variable was found) and never populate it from checkout
inference, Session ownership, claims or legacy Task-origin bytes. Provider,
tmux and SSH forwarding preserve that declaration, while Exec causation stays
with the original provider parent. No new confinement decision follows.

2026-09-30 · **Missing PATH executable.** Jack Heart's CI steer requires explicit
lock → PATH → selected installation → driver as last resort. The accepted lock
is a leading PATH directory (LOO-334), so current resolution honors it through
the same PATH lookup; `LF_BIN`/`LF_CONTROL_BIN` are historical Home pins, not a
new lock declaration. Exec `command[0]` records the actual child executable.
