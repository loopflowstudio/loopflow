# Operating Through Loopflow

Loopflow owns git, worktrees, delegation, and release plumbing. Use `lf` for
those operations so placement, release state, and execution authority stay
consistent. Read-only inspection and ordinary edits/tests run directly here.

## Execute Here First

Stay in the worktree supplied for this run. Do the assigned work here; do not
create another worktree or launch another worker merely to execute it. When
worktree management is the task, use `lf wt`, never raw `git worktree`.

Use orchestration only when the user or selected skill calls for it. Do not
guess a Wave, inspect PM, or repair auth as a prerequisite for ordinary work.
If an explicitly requested service is unavailable, report the exact blocker
and continue whatever can be completed locally.

## Git and delivery

```bash
lf commit -m "<what changed and why>"  # local checkpoint
lf rebase --plan                     # inspect integration strategy
lf rebase                            # apply it
lf pr publish --title "..."          # push and create/update PR
lf pr submit                         # prepare for the user's merge click
lf pr arm                            # prepare and request auto-merge; return
lf pr land                           # watch CI, repair, and finish merged
```

Publish is the default for making work visible; it does not rebase. Submit is
for a reviewer to land; arm/land request auto-merge. Bare land keeps the Task open;
`-c` completes it after merge, and `--next <slug>` rotates its PR chain. Use the
selected delivery skill for preparation and recovery. `lf pr open` also opens
the review page; use it only when the user asks to see the PR.

Preserve existing work before editing. Checkpoint coherent changes with
`lf commit`; never include another active contribution just because it is dirty.
Do not ask permission for reversible edits or local tests. Ask before pushing,
PR mutations, external messages or other external side effects, and destructive
operations unless already authorized by the user or selected workflow.

## Evidence Loop

Make the finish line explicit: the observable result, proof, and near-misses
that do not count. When uncertainty matters, record observations separately
from hypotheses in the working design or evidence notes.

Use the smallest safe check that distinguishes the leading explanations.
Verify against all relevant recorded evidence, not only the latest case.
Treat unexpected tool, test, or user output as a
counterexample: stop dependent steps, revise the model, then continue. Never
rewrite an observation to preserve an explanation or call a simulation live proof.

Delegate only when authorized and when an independent subset makes the problem
smaller. Keep the main blocker inline. A supplied Flow is an instruction;
follow its authored order and review boundaries.

## Speak and inspect

Write in language the user would use themselves. In persisted artifacts—Tasks,
PRs, docs, memory, reports, and decision summaries—refer to people by name.
For example, if Maya made the request, write “Maya requested the prototype path.”
In conversation with Maya, write “You requested the prototype path.”
A stored transcript remains conversation;
a summary extracted from it uses names. Refer to other participants by name.
Use names supplied in context or by the person; never infer a requester from
the machine owner, account, or Task assignee. If a necessary name is unknown,
ask when possible or leave the attribution explicitly unresolved. Do not write
“the human,” substitute an ambiguous “you” in an artifact, or claim someone's
approval without evidence.

Answer the user in this conversation. Never open another
session merely to reach them. Headless, `lf ask "<request>"` opens a durable
session and waits for completion. Respect existing authorization.

When asked about Loopflow state, use `lf ls --json`, `lf status <wave> --json`,
or `lf roadmap --json`. Do not reconstruct shared state from processes or
worktrees. Detailed placement, Task supervision, and recovery belong to the
`loopflow` and `wave/operate` skills.

Use `lf screenshot SOURCE -o OUTPUT` for unattended HTML or URL captures;
never launch a GUI browser executable for capture. Keep credentials out of
terminal output, logs, and chat; follow the repository's secret-management policy.

## Context and durable knowledge

Read the supplied repo guide and existing design before deriving another plan.
Recursive Markdown under `scratch/` enters this worktree's runs; selected Wave
context belongs to that Work. A path in another checkout does not transfer its
contents. Treat earlier drafts as evidence, not automatic approval.

Write designs to `scratch/<branch>.md` and open assumptions to
`scratch/questions.md`. Delivery preparation clears scratch: preserve useful
conclusions in their durable owner before shipping.

Put repeatable task instructions in the skill that exercises them; repo-wide
conventions in the repo agent guide; configuration in `.lf/config.yaml`.
Curate Wave decisions in `wave/<name>/MEMORY.md` through `update-wave` or
`record-learnings`. Do not create miscellaneous `.lf/` handoff notes or copy
maintainer instructions into customer skills.
