---
requires: accepted direction, and the dated chapter review when there is one
produces: each participating Wave's next plan applied, and Task candidate notes
---
Open a new chapter from direction people have agreed: plan each Wave's next Project, apply the plans, then add the new Tasks.

A chapter is the shared name of each Wave's In Progress Linear Project. Waves
keep their objective, memory and instruments across chapters. Ordinary
Projects need no chapter.

## Plan

Read the dated reviews, Wave goals and memory,
`lf wave status <wave> --no-sync --json` and `lf wave show --json`. Refer to
Projects by ID; names never select them. Missing history is unknown. Run
review-chapter first if it is needed. Direction comes before planning:
headless without it, name the missing decision and stop.

Ask each participating Wave, nested ones included, for a wave-start-chapter
proposal, giving it the direction and prior results. Proposals change nothing.
Merge them into `scratch/chapter-plan.json`:

```json
{"name":"October","waves":[{"wave_id":"<Wave UUID>","project_id":"<Project UUID>","project_name":"October — reliability","metric_targets":[],"krs":[{"text":"Observable improvement","holds":false}]}]}
```

Every plan needs written KRs. Targets go on Projects; instruments and readings
stay on Waves. Keep Task candidates apart in `scratch/chapter-candidates.md`,
grouped by destination Project: a local key, title, brief, acceptance, and the
issue ID once created.

Show the KRs, targets, carried work, kept backlog and open questions, and ask
before applying. A significant change afterwards is asked again. Record what
was said, with its date.

## Apply

```bash
lf wave edit-plan <wave> --name "October — reliability"
lf wave update-plan --wave <wave> --plan scratch/<wave>-plan.json   # {"metric_targets":[],"krs":[]}
lf wave sync <wave> --json
```

`update-plan` replaces the plan's whole content, so supply every KR and
target. `wave sync` shows what landed. After a partial failure, retry that
Wave with the same plan; one failed Wave doesn't stop another. Started work
keeps its Task, checkout, PR and Session. Unreviewed backlog stays where it is.
Planning launches nothing and completes nothing.

## Add Tasks

Only after the Projects exist, create the candidates people agreed to with
`lf task create --wave <wave> --title <title> --notes <description>`, noting
each issue ID by its candidate key. Before retrying an uncertain creation,
look for the existing issue; never create it twice. A failure here leaves the
new plans usable; keep the remaining candidates for retry.

Write each brief for someone who missed the planning conversation: what a
person is trying to do, what gets in the way, what should improve, titled with
that improvement. A short paragraph and a few acceptance bullets, keeping
exact numbers and failure cases. Possible solutions are marked as ideas. Give
Tasks outcomes useful on their own, not implementation layers.

Before `scratch/` is cleared, move lasting decisions to where they belong.
