# LOO-715 current direction

On 2026-11-14, after reviewing the incident summary, Jack Heart said “lets do it.”
This authorizes starting the existing Task and supersedes the earlier “Prepare
only; do not launch” boundary. The Task was unassigned on November 7; this is
Jack's explicit request to resume work, not an automatic operator admission.

Investigate and repair the browser-page lifetime failure and make future crashes
diagnosable. Use the existing Task checkout and incident reports. Preserve tab
history, pane moves and Undo; do not retain pages indefinitely. Establish a
reproduction and test a justified lifetime change rather than assuming a delay
fixes it. Inspect current main changes before expanding into adjacent work.

The crash-reporter choice remains open. Evaluate disabling Ghostty's reporter in
an isolated diagnostic build and the alternative of correlating existing fatal
Sentry envelopes in `lf doctor`; bring a tested recommendation to this
conversation before changing the production reporter policy.

Never crash Jack's live Desktop. Keep process-memory dumps, envelopes and their
derived private evidence outside Git and do not upload them. Record the actual
OS/WebKit version of new experiments and distinguish headless checks from the
visible-window reproduction. No macOS upgrade, Apple report, merge or install is
authorized by this request. Stop for review before shipping.
