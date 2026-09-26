> Research and iteration history. [Current accepted design](../main-view-task.md)
> governs; earlier three-loop/person-badge proposals below were superseded.

# Before the first Run

The current decision is Task 1, with no Runs started. The two later studies are
Task 2 (running, paused or blocked execution without an open Session) and Task 3
(one or more open Sessions). Do not prematurely choose their layouts from this
prestart study.

Finish: three usable website treatments in the accepted repo/Wave/Task frame,
with Start, inspectable Flow order, a tucked-away Flow selector, and collapsed
comments. Local work evidence is removed. Task title/description stay exact.
Starting only demonstrates intent; no provider, planning write or native change.

## Observed references

- [Loopflow, February 3](https://github.com/loopflowstudio/loopflow/commit/e6bb6756de75c579f00ccb6b4e69ddc97564b44d):
  inspected FlowProgressPills in the historical commit. Completed/current/future
  steps formed a compact connected sequence; current used burgundy and elapsed
  time. This supports A's footprint, not invented running evidence before Start.
- [Loopflow screenshot, February 26](https://github.com/loopflowstudio/loopflow/commit/c6930ddc3797177b1d9cb493931f26d2945751b9):
  inspected the retained native running screenshot. Burgundy navigation and a
  small Progress card are historical visual evidence. They do not imply the
  old runtime model should return. Saved under flow-references/.
- [Zapier outline](https://help.zapier.com/hc/en-us/articles/8496181725453-Learn-key-concepts-in-Zap-workflows):
  inspected the official vertical step screenshot and editor documentation.
  Step selection opens additional information/options in a sidebar. B adapts
  this to an inline expandable outline to fit the Task surface.
- [GitHub Actions graph](https://docs.github.com/en/actions/how-tos/monitor-workflows/use-the-visualization-graph):
  inspected the official connected job-card image. Job dependencies, grouped
  matrix jobs and drill-in are explicit. C adapts cards and grouping for a
  nested Code flow. The source is a run monitor; this study is a prestart preview.

Checked September 25, 2026. Third-party screenshots are reference material only.
The study includes links beside each source. No Notion/Figma workflow editor
claim is made from their unrelated document-navigation patterns.

## Design hypotheses

A should fit the calmer workspace best when the Flow is already understood.
B should help explain an unfamiliar Flow without needing a separate editor.
C should reveal nesting best but costs more vertical space and visual attention.
All three use identical local Flow definitions; no parallelism is invented.
Task design's Review design is a human boundary; Build's Review slice is not.

## Data limits

LOO-285 text is from the existing dated Linear capture. The not-started state and
default Build selection are illustrative, not claims about its current runtime
or chapter recommendation. No comments are invented from description paragraphs.
The comparison chrome declares these limits outside the proposed product UI.

## Review of the working study

Claude contributed flow-options.js/css through `lf -b -m claude`; parent owns
the frame integration, comparison/references page and browser verification.
The initial render preselected Kickoff, visually resembling a running step, and
retained the selected Task in the started sidebar. Corrected both: the study's
one Task is explicitly simulated unstarted, and steps begin neutral. Inspection
only highlights a step after a click. Replaced incomplete radio semantics with
ordinary selection buttons and restored focus after Escape/Dismiss.

Browser checks pass for A/B/C at 1400 and 1100 pixels: neutral initial state,
Flow changes, human-step detail, exact Task/Flow Start intent, collapsed/expanded
comments, returned keyboard focus and no document overflow. The original
current-data check also passes for exact titles/descriptions/objectives/KRs and
all three repository switches. Non-study mode retains the original runtime
classification and contains no Flow prototype. The first check incorrectly
expected A's expanded human-step label before selecting that step; the corrected
check inspects the actual step first. No production assertion was weakened.

Inspected final lf screenshot captures for all three; C deliberately scrolls
its contained diagram at smaller widths. No configured Run, Session or comment
behavior is proven by these simulated controls.

## Accepted C and backward-edge direction

The human prefers C. Comments moved below Description with a fixture count of
zero. The Flow name now becomes a searchable combobox; selection changes the
diagram, Escape cancels, and the neutral unstarted indicator precedes Description.
Browser checks exercise typing, keyboard/pointer selection, no matches, focus
return and diagram updates. The count is not a fetched Linear comment count.

Read the sibling `loopflow.restore-task-continuation-with-a` without edits:
loopflow.md, integration contract, cursor integration, no-pass-limits, concept
audit, current Flow YAML and RepeatPolicy/FlowDecision/reducer source. Current
source supersedes older notes describing max_iterations or Task-only loops.
A loopflow remains a Flow with backward edges, accepted by ordinary Flow entry
points. First/loop/finally become one definition. Finally means the forward
success path, not unconditional cleanup. Blocked opens help and returns evidence
to decision assessment; it neither advances a human gate nor adds a third edge.

Feature currently composes Task design and Pursue, eight steps in total:
Review design returns to Kickoff; Loop decide and human Demo each return to
Implement. There is no additional delivery node in the observed YAML. The study
captures exact source text/hashes in flow-references/sibling-flow-source.json.
C's picker adds Feature and uses it as the illustrative default, with all three
return paths and distinct human revision styling. A/B remain finite comparisons.

Inspected the final native-size browser capture. Focused browser interaction
checks pass for eight nodes, two subflows, three return paths, step inspection,
and switching Feature → Code → Feature. An initial selector also counted human
icon paths; narrowed it to the diagram's root SVG rather than changing content.
This is topology/presentation evidence, not a merge, runtime demo, or claim that
Feature is LOO-285's current configured Flow. Later execution states must use the
pinned definition and exact current occurrence, including pass history.

The human found the two-section Feature view still read as multiple Flows.
Replaced it with one continuous eight-step path. Following further feedback,
each authored return now encloses its repeated span with a softly colored rounded
background and a loop symbol. Human revision remains ochre and agent iteration
burgundy; no Task design/Pursue section labels remain. Inspected the updated
browser capture. This visual direction supersedes the preceding two-lane study.

Latest human correction: Feature has exactly one loop, Implement through Loop
decide. Kickoff/Review design belong to the once-only opening; Demo belongs to
the once-only final section. Removed both human return edges and their regions.
This accepted direction supersedes the captured sibling topology above; the
source receipt remains an unchanged observation, not the approved design.
Only the prototype and this checkout's direction changed; the sibling runtime
was not edited. Its two extra return edges need reconciliation separately.

Visual refinement: literal lowercase skill names in monospace, without per-node
descriptions. A person/You badge and stronger outline distinguish human steps.
“Loop” is centered at the top of the single repeated region; the return arrow
has no Iterate label. Step details remain available on click. Inspected the
updated screenshot and checked exact names, one region, two human badges and
Demo's once-only detail in the browser.
