---
requires: a named Wave and accepted direction
produces: exact-ID successor proposal, KRs, separate Task candidates and evidence gaps
---
Shape this Wave's next chapter from accepted direction and dated evidence.
Read its goal, memory, available review and `lf wave status <wave> --no-sync --json`.
Use exact Project IDs; names and current status never reconstruct history.
Return material challenges to the requesting conversation. Headless work lacking
accepted direction reports the missing decision and stops. Existing acceptance
remains valid.

Name the beneficiary and experienced improvement. Author nonempty KRs before
chapter creation, with targets and evidence windows where useful. Never carry
forward checked verdicts. Flow is optional; an empty string is valid. Ordinary
Project ensure has no chapter or KR requirement.

Return one entry for the retained chapter plan:

```json
{"wave_id":"<Wave UUID>","successor_id":"<Project UUID>","create":true,"project_name":"October — reliability","content":{"metric_targets":[],"flow":"","krs":[{"text":"Observable improvement","holds":false}]}}
```

Allocate a new UUID v4 once, or preserve an explicitly selected existing Project
UUID with `create:false`. Keep proposed Task candidates separately, keyed locally
and grouped by destination UUID; include title, brief and acceptance. Candidates
are optional and are admitted only after chapter Projects exist.

With a complete retained plan, preview this Wave independently:

```bash
lf wave new-chapter <wave> <name> --plan scratch/chapter-plan.json --dry-run --json
```

Return agree/challenge, exact input, dated preview and evidence gaps. Started
work carries identity and execution intact; unreviewed backlog remains in its
original Project. Unknown work stays unresolved. The operation owns classification;
do not substitute prose cancellation rules. Preserve predecessor KRs and history.

This contribution proposes only. The caller reconciles acceptance and applies
through repository or Wave new-chapter with the retained plan. Never write the
future plan into the current Project, rotate on a read, or create empty-KR
successors awaiting later planning. Separate Task admission retains candidate
notes and recovered issue IDs so failure can resume without duplicating work.

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
