# LOO-712 review and delivery

Jack Heart requested a Mitchell Hashimoto-style simulated review, repairs, and
shipping on 2026-11-02. This is an independent review using the repository's
review ritual, not a statement of Mitchell Hashimoto's views.

## Confirmed findings

- A waiting viewer processed before a disconnected controller received `busy`
  despite that controller's EOF already being readable. The owner now observes
  every connection and releases dead claims before processing requests.
- Two takeovers in one owner pass overwrote the first revocation notice. Its
  former controller stayed attached with silently rejected input. The owner now
  retains and delivers every revocation.
- Events larger than 64 KiB were recognized in one read but leaked as terminal
  bytes across smaller reads. The scanner now uses the payload limit for whole
  events, independently of the stream-frame chunk size.
- Pending events accumulated without a bound while the program left a terminal
  sequence unfinished. Pending events and queued replies now share the viewer's
  existing 4 MiB output limit, checked even before a sequence can finish.

All four regressions failed before their respective repairs.
The tests pause only their fixture owner to deliver the competing operations
together; the replacement test verifies both notices and the final controller.

Check: `uv run python scripts/test_network.py cargo test -p loopflow --test transport_tests < /dev/null` — 29 passed, including both regressions.
Check: affected gate — 75/76 passed; `provider_parentage_does_not_assign_work_outside_its_checkout` failed with a foreign capture key; rerun with inherited LF variables cleared before nextest as well as its runner. Clippy did not run after that failure.
Check: final transport source — 23 unit tests and 29 integration tests passed; the capture-ownership fixture passed with LF variables cleared before the runner; `cargo fmt --all -- --check` and `cargo clippy --all-targets --jobs 4 -- -D warnings` passed.

## Remaining delivery

- Finish static checks and affected Session boundary checks, commit and publish
  these fixes with the findings in the PR description.
- Preserve Jack Heart's landing order: LOO-705, LOO-708/LOO-709, then LOO-712.
  Sync and reconcile their final transport/lifetime changes before landing.
- Jack Heart has authorized shipping LOO-712 after review fixes. Earlier
  auto-ship was withdrawn to enforce dependency order, not to revoke this request.
- Preserve PR #1800's explicit live Claude, Desktop, SSH and installed rollover
  acceptance limits; shell fixture checks do not establish those outcomes.
