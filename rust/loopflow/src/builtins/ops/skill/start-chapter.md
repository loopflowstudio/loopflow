---
requires: accepted direction and a dated plan review when available
produces: each participating Wave's next plan applied, separate Task-candidate notes
---
Start the repository's next plans from accepted direction. Waves retain their
objective, memory and instruments. Each Wave's planning is its current plan,
one In Progress Linear Project when connected. Planning starts by authoring
each Wave's plan in place and admitting its Tasks; there is no rotation command,
so no Linear Project is completed or created here and no Task moves.

## Review and plan

Read available dated reviews, Wave goals and memory, `lf wave planning <wave>
--json` (the cached plan and Tasks) and `lf wave show --json` (plans with
history). Use stable Linear Project IDs. Missing history remains unknown; current
readings cannot reconstruct earlier outcomes. Run review-chapter when needed.
Obtain explicit acceptance of direction before scoped planning. Existing
acceptance remains valid; headless work without it reports the missing decision
and stops.

Request bounded wave-start-chapter contributions for the participating Waves,
including nested Waves. Supply accepted direction and prior evidence.
Contributions return proposals, never mutate planning. Reconcile them into one
retained `scratch/planning.json`: the planning round's name and, per Wave, its Wave
UUID, the Linear Project UUID its plan lands in, the plan's name, KRs and targets:

```json
{"name":"October","waves":[{"wave_id":"<Wave UUID>","project_id":"<Project UUID>","project_name":"October — reliability","metric_targets":[],"krs":[{"text":"Observable improvement","holds":false}]}]}
```

A Wave's plan lands in its current Linear Project. Names never select one. Every
plan needs authored nonempty KRs; Graph may be empty. Preserve existing names
and unrelated content. Targets belong to plans; instruments and observations
stay on Waves. Never carry forward checked verdicts.

Keep optional Task candidates separately in `scratch/task-candidates.md`, grouped
by exact destination Linear Project UUID. Give each candidate a stable local key, title,
brief, acceptance, and admitted issue ID when known. These are proposal notes,
not another Task store. A complete future Task inventory is not required.

Show each Wave's KRs, targets, Linear Project ID, open work and unresolved evidence.
Obtain acceptance of the reconciled plan before applying; material changes need
renewed judgment. Preserve names and dates of decisions.

## Apply each Wave's plan

```bash
lf wave edit-plan <wave> --name "October — reliability"
lf wave update-plan --wave <wave> --plan scratch/<wave>-plan.json   # {"metric_targets":[],"krs":[]}
lf wave sync <wave> --json
```

`update-plan` replaces the current plan's complete content, so supply every
KR and target. `wave sync` refreshes the plan from Linear and shows what landed;
retain its output and exit status. Retry a Wave with the same retained plan
after partial failure; a failed Wave does not block another. Started work keeps
Task, checkout, PR, Session and Graph identity; unreviewed backlog stays where
it is until explicit disposition. Unknown evidence never authorizes
retirement. Planning neither launches workers nor completes Tasks or KRs.

## Admit new Tasks separately

Only after every plan has landed, reconcile candidate notes against the exact
Linear Project IDs. Use `lf task create --wave <wave> --title <title> --notes
<description>` for accepted candidates. Record returned issue IDs alongside
candidate keys. Before retrying an uncertain creation, inspect provider issues
and recover its existing identity; do not blindly create again. Leave ambiguous
matches unresolved.

Task admission may run across all participants or incrementally. Its failure
leaves the plans usable; retain unadmitted candidates for retry. Do not treat
pending admission as successful completion.

Curate durable decisions at their owners before scratch cleanup. Repository edits
use the selected publication workflow; planning is not Git publication.

## Task briefs

Write a description someone can understand without the planning conversation.
Open with what the person is trying to do, what gets in their way, and what should
improve. Use a short title naming that improvement. Preserve useful user language;
use concrete verbs rather than process phrases such as “establish the bounded
outcome” or “settle the publication commitment”.

Usually one or two short paragraphs and a few acceptance bullets are enough.
Use headings only when they help. Describe observable success, including required
numbers, windows, failure cases, and constraints; brevity must not erase them.
Technical Tasks can serve maintainers or operators without inventing a customer.
Mark possible solutions as tentative. Keep architecture, implementation steps,
and detailed proof in the design, linked and available to the worker.

Keep the description current. Put dated progress, plan allocation, queue
changes, launch attempts, and verification updates in Task reports (`lf task report`).
A report should say what changed and what it means; link detailed receipts instead of pasting raw IDs, timestamps, or routine no-op logs. Retain
dated application evidence in existing records. Proposals draft reports without posting.
Do not use a description update or worker steering as a substitute log channel.

The thread may be collapsed by default. Keep current blockers, actual dependencies,
accepted scope, and unresolved contrary evidence summarized in the description
when they affect the work. A queue position is not necessarily a dependency.
When a steer changes the accepted scope, reconcile the description and retain
the steer as history. Do not append dated amendments or require readers to
reconstruct the current brief from the thread. Preserve decisions and evidence
before removing superseded prose.

For example, describe: “After an interrupted release, maintainers need to see
whether anything shipped and safely continue unfinished work.” Acceptance can
require that a retry never publishes twice and that failures remain visible.
Put “Moved into October's plan; scheduled after the installation repair”
in a report. Include that repair in the description only if it is a real blocker.
Give follow-up Tasks independently useful outcomes, rather than implementation layers.

Link related work where you explain its relevance. In prose and PR bodies, use
`[Title · Task ID or PR number](known URL)` on first mention; shorten later references
when unambiguous. State the relationship, such as builds on, supersedes, or verified by.
Use known URLs and preserve cited decisions and evidence somewhere that survives shipping.
In operational lists, put the ID first: `[Task ID or PR number · Title](known URL)`.

Apply these rules within the plan’s existing proposal and acceptance boundaries.
Preserve the accepted brief when applying it; retain dated application evidence;
summarize relevant changes in Task reports, never
prepend or append allocation logs to the description.
