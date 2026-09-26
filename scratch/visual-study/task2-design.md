> Historical iteration notes. [Current accepted design](../main-view-task.md)
> governs implementation, including title-level New session and named Sessions.

# Task 2 — execution, no open Session

Preserve the accepted Task 1 composition: one Feature path, literal monospace
skill names, yellow pending human steps, one named Loop around Implement through
Loop decide, Description and collapsed counted Comments. Design/Demo run once.

Show three execution situations in the website: Running, Paused, Blocked.
The observable distinction is current Run versus saved Flow position. A paused
Task has no active Run in this scenario; a blocked Task carries the reason work
cannot proceed. Neither condition means the Task is completed or its history
has disappeared. No open Session is fabricated from a headless Run.

The same diagram now marks the current occurrence in pass 3. Completed marks
refer to the opening steps and this pass's predecessors, not a permanent
completion percentage for a repeating Flow. Execution facts/actions occupy
the space between the diagram and Description. Secondary recent Run evidence
can expand. The chosen started Flow is displayed, not silently replaced by a
different definition through the prestart picker.

All execution facts, reasons, timings and interactions are labelled simulation
in the outer study. Exact captured Linear title/description remain unchanged.
Controls cannot start/stop a provider, open a real Session, or write planning.
Pause illustrates a stop at the next boundary; Resume uses that saved position.
Blocked offers help without treating acknowledgement as Flow advancement.

The execution section is a bounded Claude contribution. Parent owns graph
integration, scenario controls, review, browser checks and presentation.
Task 3, one or more open Sessions, remains the next separate study.

## Review and remaining decision

Task 2 is now presented in task2-study.html. Claude authored the execution
section through `lf -b -m claude`; parent integrated the one accepted Flow
diagram and three scenario states. No native source, PM record or provider was
changed. The human has not yet selected or accepted this execution treatment.

Review corrected disagreement after Resume: the diagram advanced to compress
while the panel's sample still named implement. Both now consume the same
simulated position, and another Pause advances the boundary to review-slice.
Recent-run samples now use actual skill names (concept-review/review-slice),
not an ambiguous review alias. Sample caveats stay in the outer study or the
expanded history rather than the primary status line.

Browser interaction checks pass for running → paused → resumed positions,
second pause, blocker retention after the Unblock preview, history expansion,
one loop/two human badges in all states, and the unchanged Task 1 prestart
picker and no-runs presentation. The initial probe queried a just-reloading
iframe before its new scenario was ready; assertions now await the visible
state instead of reading the transient DOM. No product workaround was added.
Inspected lf screenshot captures of Running, Paused and Blocked at 1440×1000.
These checks establish prototype behavior only. Pause scheduling, resumed
providers, real blocker/Ask association and configured input remain native
implementation concerns after the visual direction is chosen.

First human feedback: reserve the above-node marker for execution status.
Moved the person/You marker inside each human node, beneath its skill name,
retaining the distinct border. This applies to the shared Task 1/2 diagram.
The immediate follow-up simplifies this further: human steps use pale yellow
fill, without any person/You marker. That supersedes the inside-marker proposal.
Completed steps, including completed human steps, use pale green. Yellow is
reserved for pending human steps; the current step keeps its execution marker.
Latest color direction: Running and Loop blue, completed green, pending human
yellow, Blocked red, Paused neutral. Applied to nodes, loop region/arrow/heading,
status badges, execution summary and recent-run outcomes.
The count now sits in the loop header: “Loop · Iteration 3”, following the
human's terminology correction. Summary/history also say iteration. A tooltip
states that two iterations are completed and the third is current. Removed the
duplicate badge from the Flow toolbar; prestart still shows only “Loop”.

Hovering or focusing the started Flow name now reveals Stop & restart. Its
search selects a replacement for a confirmation panel; selection/cancel leave
the existing Flow and current step intact. Confirmation only produces a labelled
prototype intent. Keyboard focus and touch also expose the action.

New session is available in Task 1 and Task 2, independent of Start/Pause/Resume.
It previews a plain Task-context conversation in the Task's worktree without
advancing/replacing the Flow. The captured Task has no checkout path in this
snapshot, so no path is invented. Actual worktree preparation/resolution and
Session launch remain the implementation boundary. Task 3 will design its pane.

Browser interaction checks cover hover/focus visibility, typed replacement,
cancel preserving position, explicit restart intent, and New session in both
states preserving the Flow and returning keyboard focus on dismissal.
