---
requires: accepted direction and dated chapter review when available
produces: retained exact-ID chapter plan, rotation result, separate Task-candidate notes
---
Open a new chapter from accepted direction. Waves retain their objective, memory
and instruments. Each Wave's shared local binding selects its current Project;
chapters coordinate optional rotation. Ordinary Projects need neither chapter
names nor a workflow. Linear retains Project content, status and history.

## Review and plan

Read available dated reviews, Wave goals and memory, `lf wave status <wave>
--no-sync --json`, and `lf roadmap --json`. Use stable Project IDs. Missing
history remains unknown; current readings cannot reconstruct earlier outcomes.
Run review-chapter when needed. Obtain explicit acceptance of direction before
scoped planning. Existing acceptance remains valid; headless work without it
reports the missing decision and stops.

Request bounded wave-start-chapter contributions for the participating Waves,
including nested Waves. Supply accepted direction and prior evidence. Contributions
return proposals, never mutate planning. Reconcile them into one retained
`scratch/chapter-plan.json` with this shape:

```json
{"name":"October","waves":[{"wave_id":"<Wave UUID>","successor_id":"<Project UUID>","create":true,"project_name":"October — reliability","content":{"metric_targets":[],"workflow":"","krs":[{"text":"Observable improvement","holds":false}]}}]}
```

Allocate each new successor UUID v4 once and retain it. For an explicitly selected
existing destination, use its exact UUID with `create:false`. Names never select
Projects. Every destination needs authored nonempty KRs before creation; Flow
may be empty. Preserve existing names and unrelated content. Targets belong to
Projects; instruments and observations stay on Waves.

Keep optional Task candidates separately in `scratch/chapter-candidates.md`, grouped
by exact destination Project UUID. Give each candidate a stable local key, title,
brief, acceptance, and admitted issue ID when known. These are proposal notes,
not another Task store. A complete future Task inventory is not required.

Preview the exact input:

```bash
lf repo new-chapter <name> --plan scratch/chapter-plan.json --dry-run --json
```

Show KRs, targets, predecessor/successor IDs, carried work, retained backlog and
unresolved evidence. Obtain acceptance of the reconciled plan before applying;
material changes need renewed judgment. Preserve names and dates of decisions.

## Create the chapter Projects

```bash
lf repo new-chapter <name> --plan scratch/chapter-plan.json --json
```

The operation preflights every participating Wave and reserves exact destinations
before provider writes. It owns Task classification and recovery. Started work
keeps Task, checkout, PR, Session and Flow identity. Unreviewed backlog stays in
its predecessor until explicit disposition. Unknown evidence never authorizes
retirement. Rotation neither launches workers nor completes Tasks or KRs.

Retain command output and exit status. Retry with the same retained IDs and
create/existing intent after partial failure, including already settled Waves.
Never recover by name matching, create replacement UUIDs, or emulate rotation
with individual provider mutations. Conflicting external moves remain explicit.
A failed Wave does not block ordinary ensure of an unrelated Wave.

For an explicitly scoped rotation, the same input works through:

```bash
lf wave new-chapter <wave> <name> --plan scratch/chapter-plan.json --dry-run --json
lf wave new-chapter <wave> <name> --plan scratch/chapter-plan.json --json
```

## Admit new Tasks separately

Only after Project creation succeeds, reconcile candidate notes against the exact
resulting Project IDs. Confirm each Wave still selects the intended destination.
Use `lf task create --wave <wave> --title <title> --notes <description>` for accepted
candidates. Record returned issue IDs alongside candidate keys. Before retrying
an uncertain creation, inspect provider issues and recover its existing identity;
do not blindly create again. Leave ambiguous matches unresolved.

Task admission may run across all participants or incrementally. Its failure
leaves the created Projects and KRs usable; retain unadmitted candidates for retry.
Do not roll back rotation or treat pending admission as successful completion.

Curate durable decisions at their owners before scratch cleanup. Repository edits
use the selected publication workflow; operational rotation is not Git publication.

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

Keep the description current. Put dated progress, chapter allocation, queue
changes, launch attempts, and verification updates in Task comments.
A comment should say what changed and what it means; link detailed receipts instead of pasting raw IDs, timestamps, or routine no-op logs. Retain
dated application evidence in existing records. Proposals draft comments without posting.
Do not use a description update or worker steering as a substitute log channel.

Comments may be collapsed by default. Keep current blockers, actual dependencies,
accepted scope, and unresolved contrary evidence summarized in the description
when they affect the work. A queue position is not necessarily a dependency.
When a comment changes the accepted scope, reconcile the description and retain
the comment as history. Do not append dated amendments or require readers to
reconstruct the current brief from the thread. Preserve decisions and evidence
before removing superseded prose.

For example, describe: “After an interrupted release, maintainers need to see
whether anything shipped and safely continue unfinished work.” Acceptance can
require that a retry never publishes twice and that failures remain visible.
Put “Moved into the September chapter; scheduled after the installation repair”
in a comment. Include that repair in the description only if it is a real blocker.
Give follow-up Tasks independently useful outcomes, rather than implementation layers.

Link related work where you explain its relevance. In prose and PR bodies, use
`[Title · Task ID or PR number](known URL)` on first mention; shorten later references
when unambiguous. State the relationship, such as builds on, supersedes, or verified by.
Use known URLs and preserve cited decisions and evidence somewhere that survives shipping.
In operational lists, put the ID first: `[Task ID or PR number · Title](known URL)`.

Apply these rules within the chapter’s existing proposal and acceptance boundaries.
Preserve the accepted brief when applying it; retain dated application evidence;
summarize relevant changes in Task comments, never
prepend or append allocation logs to the description.
