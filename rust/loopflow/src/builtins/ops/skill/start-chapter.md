---
requires: accepted direction, and the dated chapter review when there is one
produces: each participating Wave's next plan applied, and Task candidate notes
---
Open a new chapter from accepted direction: plan each Wave's next Project, create them, then add the new Tasks.

A chapter is the shared name of each Wave's In Progress Linear Project. Waves
keep their objective, memory and instruments across chapters. Ordinary Projects
need no chapter name or workflow. Linear keeps Project content, status and
history.

## Review and plan

Read the dated reviews, Wave goals and memory, `lf wave status <wave> --no-sync --json`
and `lf wave show --json`. Refer to Projects by ID. Today's readings can't
rebuild earlier outcomes; missing history is unknown. Run review-chapter if
needed. Direction must be accepted before planning; direction accepted earlier
still counts. Headless without it, report the missing decision and stop.

Ask each participating Wave, nested ones included, for a wave-start-chapter
proposal, giving it the accepted direction and prior results. Proposals change
nothing. Merge them into one `scratch/chapter-plan.json`:

```json
{"name":"October","waves":[{"wave_id":"<Wave UUID>","project_id":"<Project UUID>","project_name":"October — reliability","metric_targets":[],"krs":[{"text":"Observable improvement","holds":false}]}]}
```

A Wave's plan lands in its current Linear Project. Names never select Projects.
Every plan needs written KRs; the Graph may be empty. Keep existing names and
unrelated content. Targets go on Projects; instruments and readings stay on
Waves.

Keep Task candidates separately in `scratch/chapter-candidates.md`, grouped by
destination Project UUID: a stable local key, title, brief, acceptance, and the
issue ID once created. These are notes, not a Task list to complete.

Preview:

```bash
lf wave edit-plan <wave> --name "October — reliability"
lf wave update-plan --wave <wave> --plan scratch/<wave>-plan.json   # {"metric_targets":[],"krs":[]}
lf wave sync <wave> --json
```

`update-plan` replaces the current plan's whole content, so supply every KR and
target. `wave sync` refreshes the plan from Linear and shows what landed; keep
its output and exit status. After a partial failure, retry that Wave with the
same plan; one failed Wave doesn't stop an unrelated one. Started work keeps its
Task, checkout, PR, Session and Graph. Unreviewed backlog stays where it is until
someone decides. Planning launches nothing and completes no Task or KR.

Show KRs, targets, carried work, kept backlog and open questions. The plan must
be accepted before applying; a significant change needs acceptance again. Record
each decision with its date.

## Add new Tasks separately

Only after the Projects exist, check the candidates against the resulting
Project IDs and confirm each Wave still points at the intended one. Create
accepted candidates with
`lf task create --wave <wave> --title <title> --notes <description>` and note
each returned issue ID by its candidate key. Before retrying an uncertain
creation, look in Linear for the existing issue; don't create it twice. Leave
ambiguous matches open.

Tasks can be added all at once or gradually. A failure here leaves the new
Projects and KRs usable; keep the remaining candidates for retry. Never call
pending Tasks done.

Before `scratch/` is cleared, move lasting decisions to where they belong.
Repository edits go through the usual PR; applying a plan is not a Git change.

## Task briefs

Write a description someone can follow without the planning conversation. Open
with what someone is trying to do, what gets in the way, and what should
improve; title it with that improvement. Keep the words people used. Use
concrete verbs, not phrases like “establish the bounded outcome”.

One or two short paragraphs and a few acceptance bullets are usually enough.
Keep required numbers, windows, failure cases and constraints. Technical Tasks
can serve maintainers or operators. Mark possible solutions as tentative; keep
architecture and implementation steps in the linked design.

Keep the description current. Dated progress, chapter moves, queue changes,
launches and verification go in Task comments: say what changed and what it
means, and link details instead of pasting raw IDs or no-op logs. Proposals draft
comments without posting. Keep current blockers, real dependencies, accepted
scope and open contrary findings in the description; a queue position isn't a
dependency. When a comment changes scope, update the description rather than
appending amendments.

For example, the description says: “After an interrupted release, maintainers
need to see whether anything shipped and safely continue unfinished work,” with
acceptance that a retry never publishes twice and failures stay visible. “Moved
into the September chapter; scheduled after the installation repair” goes in a
comment, and the repair goes in the description only if it truly blocks. Give
follow-up Tasks outcomes that are useful on their own, not implementation
layers.

Link related work where you say why it matters: `[Title · Task ID or PR number](known URL)`
on first mention in prose and PR bodies, shorter after, with the relationship
(builds on, supersedes, verified by). In operational lists, put the ID first:
`[Task ID or PR number · Title](known URL)`.
