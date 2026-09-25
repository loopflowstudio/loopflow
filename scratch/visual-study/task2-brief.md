# Task 2: execution without an open Session

Bounded Claude mockup contribution. Edit ONLY new `task-execution.js` and
`task-execution.css` in this directory. Parent owns Flow graph state, outer
study page, integration, validation and presentation. No commits or external
writes. Do not touch other files.

Create a compact execution section between the Flow and Description in the
existing C Task page. Use its warm paper, burgundy, olive/ochre and Lato styling.
This follows the accepted Task 1 UI; do not add another page title or graph.

Export window.renderTaskExecution(task, state), returning HTML. state is one of
running, paused, blocked. Parent calls it during the existing render(). The
section's root should be .task-execution. Use a unique data-execution-action
attribute for controls, not data-act/data-flow-act. CSS scoped .task-execution.
Any interaction needing to update the graph dispatches a CustomEvent on window:
`task-execution-action` with detail {action:"pause"|"resume"}. Parent handles it
by changing the simulated scenario and rendering. Keep all other interaction
local: e.g. expandable recent run history or a clearly marked prototype
message after Unblock. Never call an API, launch a provider or claim a real
Session opened. This is an explicitly labelled scenario fixture.

The three scenarios:
- Running: current Run `implement`, Claude, pass 3. State running, 2m 14s at
  this sample. No open Sessions. A Pause action previews pausing at the next
  step boundary; use title/help to explain that if needed.
- Paused: pass 3, implement finished, next step compress. No active Runs and
  no open Sessions. Resume action resumes at compress, via event above.
- Blocked: pass 3 at loop-decide after reviews. No active Runs and no open
  Sessions. Sample reason: 'The release target is unavailable. Restore access
  or choose another target before continuing.' A clear Unblock action can show
  a small prototype explanation that it would open a human conversation;
  don't pretend one already exists or advance the Flow on button click.

Prefer one compact status row and one supporting line/reason over a dashboard
of metric cards. Show what is executing, where it sits and the relevant action.
No token/cost/PID/checkout diagnostics or Local work evidence. No transcript
because there is no Session. You may include a collapsed 'Recent runs' disclosure
with a few clearly scenario-owned records, but don't fabricate a complete total
or imply this is the full invocation history. Comments already live below
Description, owned elsewhere; do not add them here. No live-looking advancing
timer or fake animated output. No extra npm dependencies.

Read flow-options.css, mockups.html and current flow-study.html for the existing
visual language. Finish with exact hook and interactions. Parent will browser
test. This is a design study, not product lifecycle implementation.
