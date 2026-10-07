# Operating Through Loopflow

Loopflow owns git, worktrees, delegation, and release plumbing. Use `lf` for
those so placement, release state, and execution authority stay consistent.
Read-only inspection and ordinary edits/tests run directly here.

- **Wave**: a durable objective with its memory and current plan of Tasks.
- **Task**: one intended outcome with its brief, checkout and PR chain.
- **Session**: one agent conversation, interactive or headless.
- **Flow**: a Task's authored sequence of steps and reviews.
- **Exec**: one actual `lf` process.

## Execute Here First

Stay in the worktree supplied for this run. Do the assigned work here; do not
create another worktree or launch another worker merely to execute it. Manage
worktrees with `lf wt`, never raw `git worktree`.

Use orchestration only when the user or selected skill calls for it. Do not
guess a Wave, inspect PM, or repair auth as a prerequisite for ordinary work.
Report an unavailable requested service and continue what can be done locally.

## Git and delivery

```bash
lf commit -m "<what changed and why>"  # local checkpoint
lf sync --plan                  # inspect integration strategy
lf sync                         # apply it
lf pr publish --title "..."          # push and create/update PR
lf submit                         # prepare for the user's merge click
lf arm                            # prepare and request auto-merge; return
lf land                           # record delivery and return
lf pr reconcile                      # check delivery once; settle a verified merge
```

Publish makes a PR ready for review; it does not sync. Submit is
for a reviewer to land; arm/land request auto-merge and return. Later reconciliation
settles verified merges and normally completes the Task. Record accepted remaining
work with `lf task follow-up` (outcome, evidence and next check); `--next <slug>`
keeps additional PR work open and rotates its chain. Use the
selected delivery skill for preparation and recovery. `lf pr open` creates or
updates a draft and opens its page; use it when the user asks to see the PR.
It preserves an existing PR's readiness. Publish/submit/arm/land make drafts ready.
A Flow ends where authored: finishing or publishing is not Task completion.

Preserve existing work before editing. Checkpoint coherent changes with
`lf commit`; never include another active contribution just because it is dirty.
Do not ask permission for reversible edits or local tests.

## Checks and Flow boundaries

Implement and compress do basic build/focused-test sanity checks. Gate owns
verification once; CI owns its matrix. Checks must run headless. Leave unavailable
checks to a capable later step and human judgment to demo/review; neither blocks
earlier work. Record a one-line result in scratch, not a verification ledger.
Fix actual failures and revise assumptions when observations contradict them.

Delegate when an independent subset makes the problem smaller. Keep the main
blocker inline. A supplied Flow is an instruction; follow its order and reviews.

## Speak and inspect

Write in language the user would use themselves. In persisted artifacts—Tasks,
PRs, docs, memory, reports, and decision summaries—refer to people by name:
“Maya requested the prototype path.” In conversation with Maya, write “You
requested the prototype path.” A stored transcript remains conversation; a
summary extracted from it uses names. Use names supplied in context or by the
person; never infer a requester from the machine owner, account, or Task
assignee. Ask for a necessary unknown name or leave the attribution unresolved.
Do not write “the human,” substitute an ambiguous “you” in an artifact, or
claim someone's approval without evidence.

Answer the user in this conversation. Never open another
session merely to reach them. Headless work that lacks required input explains
its failure in ordinary output and stops. The Wave
operator reads existing logs and discusses unresolved judgment in its ongoing
Wave chat. Taskless callers receive the failure.
Discuss review feedback in the ongoing conversation and preserve agreed direction.
Inspect execution and effect history before choosing further work. Historical
review boundaries grant no authority to close a conversation or launch a Flow.

When asked about Loopflow state, use `lf wave list --json`, `lf wave status <wave> --json`,
or `lf roadmap --json`. Do not reconstruct shared state from processes or
worktrees. Supervision, recovery and placement belong to `repo/operate`,
`wave/operate` and `task/operate`. The ongoing repository, Wave or Task
conversation is that scope's operator: it reads failed work's logs and keeps
started Tasks moving. Running Flows and optional scheduled checks continue
independently; nothing re-invokes a conversation.

Use `lf screenshot SOURCE -o OUTPUT` for unattended HTML or URL captures;
never launch a GUI browser executable for capture. Keep credentials out of
output, logs, and chat; follow the repository's secret-management policy.

## Context and durable knowledge

Keep agent progress in local working notes and the final Session response. Do not
post routine progress to Linear: Task comments are for new direction from people.
Agent comments published through `lf comment` carry a progress marker and
are excluded from steers. Use `--steer` only for deliberate new direction.
Keep `<!-- loopflow-progress:... -->` provenance when writing progress elsewhere.

Launch context has explicit budgets. An excerpt names its complete local source;
read relevant omitted sections before acting, not whole archives.

Read the supplied repo guide and existing design before deriving another plan.
Recursive Markdown under `scratch/` enters this worktree's runs. A path in another
checkout does not transfer its contents. Earlier drafts are evidence, not approval.

Write designs to `scratch/<branch>.md` and open assumptions to
`scratch/questions.md`. Delivery clears scratch: preserve useful conclusions in
their durable owner before shipping.

Put repeatable task instructions in the skill that exercises them; repo-wide
conventions in the repo agent guide; configuration in `.lf/config.yaml`.
Curate Wave decisions in `wave/<address>/MEMORY.md` through `realign`. Child
memories live in nested directories; find relevant ones with filesystem tools.
Do not create miscellaneous `.lf/` handoff notes or copy maintainer
instructions into customer skills.
