---
description: Explore an idea and capture useful Tasks for their owning Waves across repositories.
requires: none
action_style: exploratory
---
Shape intention into useful Tasks that their owners can later operate.

## Explore the idea

Start with the idea and relevant conversation or existing designs. Do not repeat
settled discovery. Explore the desired experience, alternatives, constraints and
observable success as deeply as the idea needs. Inspect code when it resolves a
real uncertainty. Preserve ambition; a technical design is not a filing requirement.

Favor larger, cohesive Tasks with clear behavioral promises and benefits. Choose
boundaries thoughtfully: useful pipelining or parallelism can justify a split,
but neither is a fragmentation rule. Keep speculative implementation steps inside
the Task or its design. A recommendation may need no PR. Prefer separate Tasks
for independently deliverable outcomes, stacked where dependent; continued work
in one Task may publish several PRs in sequence. Several Tasks are supported, never a quota. Later proof of an accepted
outcome can use another ordinary Task, with its source and expected evidence in
the brief.

## Resolve ownership

The launch repository and Wave are starting clues, not filing boundaries. Correct
initial attribution as understanding develops. Each Task has one owning Wave;
distinct outcomes may span Waves and repositories. Do not split a cohesive outcome
merely because it touches multiple repositories. Preserve dependencies and shared
context when several Tasks are appropriate.

In each relevant destination repository, run `lf wave list --json` and
`lf wave status <wave> --json` to inspect objectives, current plans and overlap.
Use that repository as the working directory for every scoped CLI call. Same-named
Waves in different repositories are different destinations. Reuse or refine an
existing Task for the same work rather than filing a duplicate.

## Capture clear intent

File once intent is clear. Show the concrete brief in the conversation and capture it;
do not impose another confirmation ritual.
Exploratory possibilities remain proposals.

Each Task must stand alone for its owner: include the problem, beneficiary,
desired outcome, observable acceptance, real constraints and consequential open
decisions. Separate accepted choices from possible mechanisms. Attribute requests
and decisions using people's known names; leave unknown attribution unresolved.
Keep implementation sequencing in a design when one exists, but include essential
intent in the Task itself. A transcript or path in another checkout is insufficient.
For dependent Tasks, prepare a self-contained child-specific design excerpt with
its own scope and acceptance; never copy a common plan and toggle “current slice.”
The later operator transfers it with `lf checkout <child> --stack-on <parent>
--design <path>` before launch. Capture itself still launches no work.

From the destination repository, file with `lf task create --wave <owner>` without
`--run`. Supply the title and complete brief through `--title` and stdin or
`--notes`; use `lf task edit` for refinements to a confirmed Task.
Return each actual Task link, repository and owning Wave.

If the repository is inaccessible, ownership unresolved, or its Project unavailable,
leave an explicit unfiled brief in the conversation. Report provider failures
honestly. After an uncertain write, reconcile with `lf wave status <wave> --json`
in that destination. Every explicit create makes a distinct Task, even with
identical input; only internal synchronization retries reuse the saved identity.
Recover the created Task's identity and refine it with `lf task edit`. If the
write remains uncertain, report it rather than issuing another create.
Report each Task separately. Preserve successful filings when
another fails; never recreate the batch or substitute the launch repository.

## Keep the conversation available

Capture does not start workers or supervise captured Tasks. Stay in this Session's
original repository/Wave scope for further ideas; do not bind it to the first Task,
move its checkout, or redirect Desktop focus after filing. Destination-scoped CLI
calls do not relocate the conversation. The owning Waves operate their Tasks.
When used inside an existing Wave conversation, follow this skill there; keep operating
that Wave's Tasks while capturing new work.
