# LOO-494 Codex startup repair

Jack Heart authorized measured diagnosis through release and startup acceptance.

## Causal finding and design — October 16, 2026

A benchmark-created Codex Session with one native turn reopened in 0.77 seconds
on a private copy of the installed database (3,790 Sessions, 788,632 Processes,
2,033,513 events; copied account routes removed). With 234,483 synthetic process
receipts, a cleanly detached reconnect still reached its editable prompt in
1.47 seconds. Abruptly terminating only the benchmark client's process group
left its prior attachment to recover. That actual reconnect stalled at Preparing
codex; 789/789 sampled main-thread stacks were in
resume_session_agent_process → process_evidence → read_process_receipt_files_at.
This establishes the reproduction's cause, not a retrospective stack for the
original 38-second observation. No context or native history was removed.

Read the requested Process's existing ID-keyed receipt directly. Preserve the
shared recorded identity rule, trace matching, PID birth checks, and attachment
fencing. Missing receipts retain the existing completion/boot fallback;
unreadable or malformed evidence stays unknown. No receipt cleanup or cache.

Delete — do not maintain: process_evidence's whole-directory receipt inventory.
Keep inventory reads for top/cleanup, where the caller actually requests all.

Checks: cargo test --release -p loopflow --lib journal::tests:: — 20 passed; network-isolated runtime/top/Task-execution suites — 13 passed; cargo fmt / all-target Clippy / benchmark Ruff and fixture pytest — passed.

Delivery constraint: Desktop sets LF_BIN to its bundled helper. Supported CLI-only
installation preserves it; full installation stops Desktop and bundled clients.
After shipping, verify the installed CLI independently and retain any unavailable
Desktop adoption as a blocker rather than interrupting live clients.

Remaining: matched release-build crash/reconnect timings, small/large native
context/history cases, exact-read safety regression, affected checks, PR/CI,
authoritative merge, supported release/install preserving clients, released
independent startup acceptance, then Task completion. Private fixtures/profiles
live at /Users/jack/benchmarks/startup-20261016; publish sanitized evidence only.
