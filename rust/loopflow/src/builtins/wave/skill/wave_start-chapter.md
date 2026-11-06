---
requires: a named Wave and accepted direction
produces: this Wave's next plan proposal, KRs, separate Task candidates and gaps
---
Shape this Wave's next chapter from accepted direction and dated findings.

Read its goal, memory, any review, and `lf wave status <wave> --no-sync --json`.
Use exact Project IDs; names and current status never rebuild history. Raise
real disagreements with the conversation that asked. A headless run without
accepted direction names the missing decision and stops. Earlier acceptance
still stands.

Name who benefits and what gets better for them. Write nonempty KRs before the
chapter exists, with targets and measurement windows where useful. Never carry
a checked verdict forward. The Graph is optional; an empty string is valid.
Ordinary Projects need no chapter or KRs.

Return one entry for the chapter plan:

```json
{"wave_id":"<Wave UUID>","project_id":"<Project UUID>","project_name":"October — reliability","metric_targets":[],"krs":[{"text":"Observable improvement","holds":false}]}
```

The plan lands in the Wave's current Linear Project (`lf wave planning <wave> --json`
names it). Keep Task candidates separate, keyed locally and grouped by
destination UUID, each with title, brief and acceptance. Candidates are optional
and are filed only after the plan has landed.

Return agree or challenge, the exact input and any gaps. Started work keeps its
identity and runs; unreviewed backlog stays in its Project; anything uncertain
stays open. Keep earlier KRs and history.

This step only proposes. The caller confirms acceptance and applies it with
`lf wave edit-plan`, `lf wave update-plan --plan` and `lf wave sync`. Never write
the future plan into the current Project from a read, or leave a plan with empty
KRs to fill in later. Filing Tasks keeps candidate notes and created issue IDs so
a failed run resumes without duplicates.

## Task briefs

Write a description someone can follow without the planning conversation. Open
with what someone is trying to do, what's in the way, and what should improve.
The title is short and names that improvement. Keep useful phrasing from the
conversation; use concrete verbs, not phrases like “establish the bounded
outcome”.

One or two short paragraphs and a few acceptance bullets are usually enough.
Say what success looks like, including required numbers, windows, failure cases
and constraints; short must not drop them. Technical Tasks can serve
maintainers or operators without a made-up customer. Mark possible solutions as
tentative. Architecture and implementation steps belong in the linked design.

Keep the description current. Dated progress, chapter moves, queue changes,
launch attempts and check results go in Task comments. A comment says what
changed and what it means; link details instead of pasting raw IDs, timestamps
or no-op logs. Proposals draft comments without posting them. Don't use
description edits or worker steering as a log.

Comments may be collapsed, so the description carries current blockers, real
dependencies, accepted scope and open contrary findings that affect the work. A
queue position isn't necessarily a dependency. When a comment changes accepted
scope, update the description and leave the comment as history; never append
dated amendments for readers to piece together. Before cutting superseded prose,
make sure its decisions and findings are kept somewhere.

For example, the description says: “After an interrupted release, maintainers
need to see whether anything shipped and safely continue unfinished work.”
Acceptance can require that a retry never publishes twice and failures stay
visible. “Moved into the September chapter; scheduled after the installation
repair” goes in a comment; put that repair in the description only if it really
blocks. Give follow-up Tasks outcomes that are useful on their own, not
implementation layers.

Link related work where you explain why it matters. In prose and PR bodies, use
`[Title · Task ID or PR number](known URL)` on first mention and shorten after;
say the relationship (builds on, supersedes, verified by). In lists, put the ID
first: `[Task ID or PR number · Title](known URL)`. Use only known URLs.
