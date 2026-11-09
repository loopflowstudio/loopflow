A dropped server connection previously ended every Desktop watch and forced a complete reading on reconnect. Watches with a baseline now retain their stream and queued readings while reconnecting, avoiding another full payload when the data is unchanged.

<!-- loopflow:task-pr-context:start -->
> [!NOTE]
> **Task:** [Resume server watches from the last checkpoint after a reconnect · LOO-775](https://linear.app/loopflow/issue/LOO-775/resume-server-watches-from-the-last-checkpoint-after-a-reconnect)
> **PR lifecycle:** The pull request is published for review; no Task settlement is requested.
<!-- loopflow:task-pr-context:end -->

## What changes

- Adds opaque reading checkpoints and `subscribe.resume`. Matching tokens omit the unchanged baseline; changed or unknown tokens, including those from a restarted server, receive a complete current reading. No disconnected watch or payload history is retained.
- Desktop keeps queued readings with their token, retries the newest unanswered read confirmation, and cancels recovery when its observation ends. Writes never retry. Watches without a baseline, refused subscriptions, queue overflow and failed reconnects use the existing fresh-watch recovery path.
- Updates Rust, Swift, fixtures, schema and documentation to protocol revision 3. Both client and server must support revision 3; older clients receive an incompatibility error.

Resume restores current state, without replaying intermediate changes. Numeric confirmations still receive readings, unavailable readings still arrive, and terminal subscriptions still end even when their final unchanged reading is omitted.

## Checks

Gate passed November 9 after syncing with main, using Rust stable 1.99.1:

- Changed-aware gate: 108 Rust tests covering server protocol, subscription identity, DTOs, served reads, live watches and terminal recovery; 73 website browser/accessibility tests; 783 headless Swift model/view tests. Architecture, formatting, Clippy with warnings denied and Swift platform boundaries passed. Four suites completed in 491 seconds.
- Both isolated macOS installation checks passed: service registration migration respected the disabled choice, and signed installed-helper replacement retained the existing owner and Graph.
- Socket-loss tests cover recovery before consuming the queued baseline and coalescing 100 unanswered confirmations. The production Work-model test retained its last reading during an outage, applied offline edits after server replacement, received later edits, and cancelled during another outage without reopening a watch.
- Real Unix-socket QA passed unchanged/changed/unknown resume, restored state without replay, independent selections, an empty list, a 61,440-byte body, cancellation, server restart, lost terminal `ended`, and revision-2 refusal.
- The first gate attempt stopped before testing when shared uv-cache inventory exceeded its 15-second deadline. A direct scan then completed in 12.13 seconds at 13.6 GiB, below its 16 GiB limit; the retry passed resource preflight and postflight without changing limits. `git diff --check` passed.

Mounted Desktop and phone recovery and payloads near the 32 MiB limit remain unverified. CI owns the full Rust matrix and Linux installation checks.
