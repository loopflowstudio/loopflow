# Desktop server cutover — November 4, 2026

Jack Heart requested deletion of the Swift/C Loopflow bindings, migration to the
server protocol, correctness checks and performance measurements. The configured
native replay below is operator evidence, not Jack's acceptance or shipping approval.
Design: [mobile-backend.md](mobile-backend.md). Measurements:
[server-performance/results.md](server-performance/results.md).

## Observed result

The SwiftPM-built native Desktop links no Loopflow FFI. A private Machine and
local Git repository used the configured Codex provider, without copying or
reading credentials. Work views loaded through the server. Opening the repository
conversation prepared it through the server, then attached its ordinary owner.

Codex remembered `LOO593-SERVER-814`. The operator entered
`UNSENT: repeat the remembered marker and run pwd. Do not change files.` without
submitting it, then stopped only the private server. Native chrome showed
Reconnecting and stale Work errors; the complete draft remained visible.

Starting the same signed helper again gave server incarnation `512debe6-fee3-4c60-8e34-e5cc7b71052c`
in place of `b8d27817-2255-490f-b8cf-40b072aded2b`. The terminal and Work watches recovered
automatically. The operator submitted the retained draft once. Codex returned the
marker and ran pwd in `/Users/jack/.lf/demos/loo593-server-5r7obf0c/prototype.repo`. Session `71b01cbc0713449eb1197a465305f2a5` and agent process
`e294e97f-0c0a-4c1c-a26c-9176fb0529e6` were unchanged. Cmd-K → Rename → Save changed the title
to **Server continuity verified** through the real server command.

This proves server loss/recovery, retained live conversation/draft, ordinary
native reads, subscription refresh, Session preparation, pane persistence and
rename on this build. It does not prove the separate LOO-582 create/bind repair,
Linux supervisor replacement, mobile integration or a released installation
upgrade. The pane-closure race found by tests was repaired after this app was
packaged; its separate Rust/Swift checks cover that follow-on change.

## Replay and evidence

App: `/Users/jack/.lf/demos/loo593-server-5r7obf0c/Loopflow Server Cutover.app`. This is an isolated **Live** app, not the installed Loopflow
or the older LOO-582 demo. It is left open for review.

Evidence root: `/Users/jack/.lf/demos/loo593-server-5r7obf0c`. Screenshots: `before-restart.png`, `during-restart.png`,
`recovered-draft.png`, `continued.png`, `renamed.png`. CLI receipts:
`sessions-before.json`, `sessions-after.json`, `sessions-renamed.json`,
`server-before.json`, `server-after.json`. The private server's current PID is
70799; restarting it does not imply stopping the owner.

Initial packaging failed because the copied SwiftPM executable lacked its Sparkle
framework rpath. The corrected bundle adds its ordinary Frameworks path. A second
setup attempt mixed the source-built and re-signed helper hashes, correctly
surfacing unavailable; `setup-failure.png` retains it. Selecting the same signed
helper for the server and Desktop repaired the fixture. The initial long bundle
identifier also exceeded the app-owned automation socket limit; the final short
identifier is `com.loopflow.demo593`. No released installation was changed.

## Verification and review

The real server/CLI equivalence test checks every shared read and write, first
subscription frames, Process recording and effects. Its Task-save payload exceeds
64 KiB, proving stdin forwarding and the expanded request bound. Swift tests
exercise server availability, cancellation, checkpoints and explicit errors.
Packaging tests and symbol inspection verify the embedded implementation is gone.
The final check results are recorded with this change's review notes.

A Mitchell Hashimoto-style simulated review found concrete improvements: delete
the duplicate mutation executor, app-only store mode and call-scoped identity;
forward stdin concurrently with output; preserve stdout on failed CLI responses;
remove the socket mailbox address atomically before cleanup so writers cannot
race pane closure. The measured write latency cost remains explicit; no fallback
or second mutation implementation was retained to conceal it.

The native demo is available for review. No merge, Auto-Ship or Task completion
was requested or performed.
