# Shared active Runs — implementation, 2026-09-24

The current internal slice adds `lf runs --active [--task …]`, required Rust/Swift
DTOs and the RegistryQuery reader. The app has not started polling it. Monitor
panes, retained Task pane choice, both native performance scenarios, bounded
native discovery and full Task acceptance remain unfinished.

Capture intervals establish exact Run-to-Exec attribution only during the capture.
Verified providers below that Exec establish liveness; native client receipts
supply direct Run ownership. Shared Work resolution now lives with Run records
and serves both Session and active-Run projections. Multiple observations collapse
to one Run. Missing ownership and traversal/read failures produce gaps; unfinished
metadata is never sufficient liveness evidence.

The concurrent discovery review reproduced a real false empty for an existing
native client without a new marker. Its correction and regression are preserved;
the native marker writer was deleted. Existing native receipts remain authoritative.
This requires walking Run directories and does not satisfy the accepted bounded
live-discovery cost. Preserve old clients while settling that cost before frequent
Monitor polling; do not move this core obligation to an optimization follow-up.

Focused verification passes against the source hashes in
[the receipt](active-runs-evidence/receipt.json): five active-Run Rust tests,
one Rust DTO fixture, one real CLI test covering both initial launch and native
resume, and one Swift DTO test. `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --all --check` and `git diff --check` pass. The final source hash
comparison found no executable changes during verification.

The native fixtures use owned `/bin/cat` children and the CLI uses an isolated
provider stand-in. They prove process/identity behavior, not configured-provider
input, native pane rendering or human acceptance. The 2020 Run date is exercised
through the real CLI; same-checkout Task isolation and dead-client removal use
the shared snapshot reader with actual owned processes.

Review changed the implementation: direct native receipts no longer require a
journaled Exec or a newly written interval; redundant native binding writes were
deleted. Missing live Exec receipts and source-read failures remain explicit gaps.
Run attribution was moved out of Session projection rather than copied. Native
receipt traversal is intentionally still an open core cost, not a performance win.
The concurrent review's legacy discovery correction remains its contribution.

No Monitor/pane implementation, performance result, app/provider interaction,
publication, Task completion or human acceptance is established by this slice.
