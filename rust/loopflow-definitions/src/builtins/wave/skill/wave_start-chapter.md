---
requires: a named Wave and accepted direction
produces: this Wave's next plan proposal, KRs, separate Task candidates and gaps
---
Shape this Wave's next chapter from the direction people agreed and the dated findings.

This step only proposes; the caller asks, then applies. Read the Wave's goal,
memory, any review, and `lf wave status <wave> --no-sync --json`. Use exact
Project IDs. Raise real disagreements with the conversation that asked. A
headless run without direction names the missing decision and stops.

Name who benefits and what gets better for them. Write real KRs, end states
that would show the bet holds, with targets and measurement windows where they
help. Never carry a checked verdict forward, and never leave KRs empty to fill
in later.

Return one entry for the chapter plan:

```json
{"wave_id":"<Wave UUID>","project_id":"<Project UUID>","project_name":"October — reliability","metric_targets":[],"krs":[{"text":"Observable improvement","holds":false}]}
```

`lf wave plan <wave> --json` names the Project the plan lands in. Keep Task
candidates apart, each with a local key, title, brief and acceptance; they are
optional and filed only after the plan has landed. Write each brief for
someone who missed this conversation: what a person is trying to do, what gets
in the way, what should improve.

Return agree or challenge, the exact input and any gaps. Started work keeps
its identity and runs; unreviewed backlog stays in its Project; anything
uncertain stays open.
