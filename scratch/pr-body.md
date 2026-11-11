Local terminals could exit after laptop sleep while their providers kept running, and live Task views could show unknown state or omit existing work. This change keeps heartbeat deadlines from counting sleep, reconnects terminals to the same running execution, and keeps the Task pane's full data through live updates.

- Use monotonic uptime for heartbeat intervals and deadlines on both sides of the connection. Detached Session retention still counts wall time.
- Recover acknowledged local terminals without restarting providers, replaying outage input, or taking another viewer's input and geometry claims. A failed automatic reconnect leaves Ctrl+R retry and Ctrl+] close in the terminal. Initial attachment and command pipes retain bounded failure.
- Subscribe `lf task view` to the full Task pane reading its renderer expects.

Reported trigger: “after i closed my laptop and reopened in a new place (where i briefly didnt have wifi)”. The four original Codex processes were still alive after the terminal errors. An isolated Task-pane regression reproduced unknown fields and missing conversations with the installed app, and retained the details after a live update with this build.

Validation: seven focused transport tests passed, including local recovery, newer-viewer claims, unavailable-owner retry, exact delivered input, heartbeat expiry, quiet connections, initial attachment timeout and stalled writes. Three focused Task-view tests passed. Formatting and `cargo clippy --all-targets -- -D warnings` passed; Clippy used the existing pinned Ghostty archive after its fresh download could not connect. The unit-test linker repeated its existing oversized `__eh_frame` warning.

The recovery test pauses a fixture relay and closes its socket; it does not suspend the laptop. After installing a build containing this change, verify on macOS that a local Session remains usable or reconnects to the same provider after lid sleep/wake without replaying input. The running app has not been updated by this work.

The `lf open` redesign is a separate auto Task, `25213d` (Simplify lf open around conversations and presentation modes). The observer/dispatch-lock contention finding is also outside this change.
