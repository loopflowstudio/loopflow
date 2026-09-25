# Accepted workspace prototype

```sh
uv run --no-project python -m http.server 8317 --bind 127.0.0.1
open 'http://127.0.0.1:8317/scratch/visual-study/task3-study.html#multiple'
```

Implement [the current design](../main-view-task.md). The accepted combination
is frame **A**, Task header **A**, Flow **C**, and named Session drill-down.
[accepted-reference.json](accepted-reference.json) records the exact source hashes.

Later human clarification accepts the continuation branch's second decision
loop after demo. The preserved prototype still shows the earlier single-loop
sample; use it for visual treatment and the current design/real pinned Flow for
topology. Do not omit that second return edge during native implementation.

| Inspect | Open on localhost:8317/scratch/visual-study/ |
| --- | --- |
| Wave objective, Current KRs and full Tasks | [Current planning](mockups.html?v=a&population=current&repo=loopflow&task=none) |
| Unstarted Task, Flow picker and Start | [Flow C directly](mockups.html?v=a&population=current&task=LOO-285&flowstudy=c) |
| Running, paused and blocked, no open Sessions | [Task 2](task2-study.html#running) |
| One Session, enters directly | [Task 3 / one](task3-study.html#one) |
| Several named Sessions, Flow and independent | [Task 3 / multiple](task3-study.html#multiple) |
| 20 started Tasks across Waves, 50 per Wave | [Density fixture](mockups.html?v=a&population=large) |

Select a Session to drill down Wave → Task → Session. Select an ancestor to go
back; use the final breadcrumb to switch Sessions. Description stays on Task.
Each Session retains its own draft. New session sits beside the Task title.
No visible Session-with-context mode or ELSEWHERE badge is part of the target.

## Source map

| Files | Use as reference for |
| --- | --- |
| [mockups.html](mockups.html) | Shared frame A, scoped repo header, started-work sidebar, Wave/Task pages, exact selection and retained sample drafts |
| [flow-options.js](flow-options.js), [CSS](flow-options.css) | Flow C visual treatment and literal nodes; older single-loop sample, searchable name, Start/restart/New session intents, collapsed Comments |
| [task-execution.js](task-execution.js), [CSS](task-execution.css) | Running/paused/blocked summary, occurrence, iteration and recent Runs |
| [task2-study.html](task2-study.html) | Explicit execution scenario harness |
| [task-sessions.js](task-sessions.js), [CSS](task-sessions.css) | Named Session list/breadcrumb, membership and retained terminal-style surface |
| [task3-fixture.js](task3-fixture.js), [task3-study.html](task3-study.html) | Explicit one/three-Session examples and integration |
| [current-plan.js](current-plan.js), [current-data.js](current-data.js) | Captured shared planning only; source receipts in [current-data-evidence](current-data-evidence/source.json) |
| [large-population.js](large-population.js) | Separate synthetic density population |

Session naming policy in the design includes initial skill/musical-animal names
and agent improvement of generated names. The browser uses prewritten subject
names and local rename only; it does not run a generator or persist shared titles.
The implementation must use shared Session identity/title ownership, never this
in-memory fixture model or the website transcript as a native replacement.

Current data is the dated September 25 capture, not a continuously live view.
Loopflow and Etude each have six captured Tasks; Kata chapter plans are unavailable.
Titles, Markdown descriptions, objectives and KRs retain their captured content.
Task 1 overrides runtime to unstarted; Task 2/3 add simulated execution. Task 3
adds simulated Session names, membership, messages and statuses. Comments (0)
is a fixture, not a fetched count. Buttons change only this prototype. No provider,
PM write, native surface or real terminal is connected.

Prototype checks prove interaction only. Native focus, process/input retention,
shared rename durability, configured lifecycle controls and performance require
the design's implementation proofs. Existing evidence: [Flow](flow-research.md),
[Task 2](task2-design.md), [Task 3](task3-design.md).

The [original comparison](index.html), [header comparison](header-study.html)
and [Flow comparison/references](flow-study.html#c) retain exploration history.
Their alternative layouts and older generic sample behavior are not additional
implementation requirements. [Workspace research](../research-workspace-design-603c72da.md)
records Notion/Figma/Claude Desktop sources; Flow research includes historical
Loopflow, Zapier and GitHub Actions references. Third-party imagery is inspiration,
not product artwork.
