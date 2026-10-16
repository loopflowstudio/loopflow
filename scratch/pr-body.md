Reopening a Codex Session after its previous client was interrupted scanned every process receipt while displaying “Preparing codex.” Read the existing Process-ID-keyed receipt directly, preserving trace/PID birth checks, unknown evidence and attachment fencing.

On the same populated fixture with 234,483 synthetic historical receipts, matched native reconnect medians fell from **53.72 to 1.85 seconds** for a short history and **54.35 to 1.79 seconds** for a 56,766-byte long context. All 12 samples retained native identity, captured input and every pre-existing event. No context, history or receipts were deleted.

[Diagnosis, raw numbers, fixture/build hashes and limitations](scripts/benchmarks/agent-startup/20261016/README.md). The original 38-second incident was not sampled; this is a controlled reproduction of the same delayed reconnect stage. These are warm shared-host results, not LOO-461 cold-cache acceptance.

Validation: 20 journal tests; 13 affected attachment/status/Task-execution tests under external-network isolation; all-target Clippy; Rust formatting; benchmark Ruff; fixture isolation pytest. The exact-read regression works without permission to enumerate the directory and retains missing/corrupt/mismatched/reused-identity behavior.

Task completion remains pending release and independent startup acceptance. CLI-only installation preserves active Desktop clients but cannot update their explicitly pinned bundled helper; Desktop adoption must not interrupt live work.
