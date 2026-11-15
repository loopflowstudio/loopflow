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

On 2026-11-14, after both isolated reporter experiments and the measured
recommendation were presented, Jack Heart replied “sure”: disable Ghostty's
crash reporter in the production artifact and rely on the existing signal
journal plus macOS crash reports. The production policy is now selected.
Implement the build/package/pin change with correct provenance and notices;
remove the unselected test-only envelope-correlation candidate. Reuse the
completed experiment evidence rather than repeating the same faults.

The WebKit lifetime repair remains separate and unfinished. Check current
display readiness before further trials; the previous display was asleep and
provided no unoccluded animation callbacks. Missing rendering must remain
unavailable, not a passing crash reproduction. Do independent reporter work
without waiting for that display dependency.

Never crash Jack's live Desktop. Keep process-memory dumps, envelopes and their
derived private evidence outside Git and do not upload them. Record the actual
OS/WebKit version of new experiments and distinguish headless checks from the
visible-window reproduction. No macOS upgrade, Apple report, merge or install is
authorized by this request. Stop for review before shipping.

On November 14, comment `f8ddfe05-c76d-4456-853f-d99a9ca43d97` relayed:
“Stop the visible browser-lifetime sweep immediately and launch no more visible
windows or Dev Desktop without discussing it here.” Background artifact
publication/build/tests continue. The sweep had completed when the steer arrived;
its parent exited and no harness child remained. No more visible trials are
selected. The missing failing baseline remains unresolved.
