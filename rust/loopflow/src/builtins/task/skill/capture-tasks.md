---
description: Explore an idea and file useful Tasks in the Waves they belong to, across repositories.
requires: none
action_style: exploratory
---
Turn an idea into Tasks the right Waves can pick up later.

## Explore the idea

Start from the idea and any conversation or design behind it; don't redo settled
discovery. Explore the desired experience, alternatives, constraints and what
success looks like, as deeply as the idea needs. Read code when it settles a real
question. Keep the ambition; a Task doesn't need a technical design to be filed.

Favor larger, cohesive Tasks with a clear promise of what changes for whom.
Split for independently deliverable outcomes, stacked where dependent, or where
pipelining or parallelism truly helps; one Task can still publish several PRs.
Keep speculative implementation steps inside the Task or its design.

## Find where each Task belongs

The repository and Wave you started in are clues, not limits. Each Task
belongs to one Wave; distinct outcomes may span Waves and repositories, but
don't split a cohesive outcome just because it touches several repositories.

In each destination repository, run `lf wave list --json` and
`lf wave status <wave> --json` to read objectives, current plans and overlap.
Run every scoped command from that repository. Same-named Waves in different
repositories are different Waves. Refine an existing Task for the same work
instead of filing a duplicate.

Plans don't show which open PRs already rewrite the code. Before filing work
against specific files, list the open PRs that change them
(`gh pr list --json number,title,headRefName,files`). The brief names each
overlapping PR, its Task and what it changes there; a choice that Task leaves
open stays open, and the brief says so rather than deciding it.

## File clear intent

File once intent is clear. Show the brief in the conversation and file it; don't
add another round of confirmation.

Each Task stands alone: the problem, who benefits, the desired outcome, how to
tell it's done, real constraints and open decisions that matter. Keep what
people asked for, quoted, apart from possible mechanisms. Keep implementation sequencing in a
design when one exists, but put the essential intent in the Task itself; a
transcript or a path in another checkout doesn't travel. For dependent Tasks,
write each child its own design excerpt with its own scope and done-when; never
copy a shared plan and toggle "current slice." Whoever launches it later passes
it with `lf checkout <child> --stack-on <parent> --design <path>`. Filing
launches no work.

From the destination repository, file with `lf task create --wave <wave>`
without `--run`. Pass the title with `--title` and the full brief on stdin or
with `--notes`; refine a filed Task with `lf task update`. Return each Task as one
link from `lf --task <issue> open --link --format markdown`
(`[ID · title](loopflow://…)`), with its repository and Wave; keep the provider
issue link separate. Run `lf open '<url>'` only when asked to open a Task now,
and report what it printed; a printed link opens nothing. Opening means the
Task's conversation: agent calls use a sibling tab, and an already-open Session
reports its location with `--focus` to reveal it. Never retry with `--here` inside
a tool call or substitute workflow inspection for a requested conversation.

If the repository is unreachable, the Wave unclear or its Project unavailable,
leave the unfiled brief in the conversation and say why. Report provider
failures as they are. Every `lf task create` makes a new Task, even with
identical input, so after a write that may have failed, check
`lf wave status <wave> --json` in that repository before anything else. If the
Task exists, refine it with `lf task update`; if you still can't tell, report it
rather than creating again. Report each Task separately. Keep successful filings
when another fails; never refile the batch or fall back to the starting
repository.

## Stay available

Filing doesn't start or watch work. Stay in this Session's original repository
and Wave for further ideas; don't bind it to the first Task, move its checkout,
or switch Desktop focus after filing. Inside a Wave conversation, keep operating
that Wave's Tasks while filing new ones.
