---
requires: a named Wave and accepted direction
produces: this Wave's next plan proposal, KRs, separate Task candidates and evidence gaps
---
Shape this Wave's next plan from accepted direction and dated evidence.
Read its goal, memory, available review and `lf wave planning <wave> --json`.
Use exact Linear Project IDs; names and current status never reconstruct history.
Return material challenges to the requesting conversation. Headless work lacking
accepted direction reports the missing decision and stops. Existing acceptance
remains valid.

Name the beneficiary and experienced improvement. Author nonempty KRs, with
targets and evidence windows where useful. Never carry forward checked
verdicts. Graph is optional; an empty string is valid.

Return one entry for the retained plan:

```json
{"wave_id":"<Wave UUID>","project_id":"<Project UUID>","project_name":"October — reliability","metric_targets":[],"krs":[{"text":"Observable improvement","holds":false}]}
```

The plan lands in the Wave's current Linear Project (`lf wave planning <wave> --json`
names it). Keep proposed Task candidates separately, keyed locally and grouped
by destination UUID; include title, brief and acceptance. Candidates are
optional and are admitted only after the plan has landed.

Return agree/challenge, exact input, dated evidence and evidence gaps. Started
work carries identity and execution intact; unreviewed backlog remains where it
is. Unknown work stays unresolved. Preserve the current plan's KRs and history
in the report; the caller replaces the content.

This contribution proposes only. The caller reconciles acceptance and applies
it with `lf wave edit-plan`, `lf wave update-plan --plan` and `lf wave sync`.
Never write the future plan into the current plan from a read, or author
empty-KR plans awaiting later planning. Separate Task admission retains
candidate notes and recovered issue IDs so failure can resume without
duplicating work.

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
