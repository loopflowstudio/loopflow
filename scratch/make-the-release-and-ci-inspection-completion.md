# Complete delivered work with accepted historical uncertainty

Jack Heart authorized this change and autonomous delivery on October 5, 2026.

Add `lf task complete ISSUE --accept-unknown-exec ID --summary REASON`.
Record the explicit acceptance in Task history, scoped to that Task and exact
Exec. Only completion consumes it. Exec outcome, process evidence and ownership
remain untouched; checkout cleanup, admission and interruption retain their checks.
Accept only missing process receipts unrelated to current Flow/Session ownership;
observed live or unresolved identities and ordinary unfinished work still block.
Recheck those conditions when consuming acceptance, including after retries.
No schema change: Task events already own durable Task decisions.

Delete — do not maintain: completion's unconditional treatment of explicitly
accepted historical uncertainty as unfinished product delivery. Preserve the
strict existing check for cleanup, abandonment, restoration and execution.

Implemented the flag, typed Task-history event and completion-only reader.
No migration or alternate execution authority was needed. Review retained strict
cleanup and admission checks, rejected acceptance of the current command itself,
and replaced a whole-history scan with a filtered SQLite event projection.
Placement initialization is checked before acceptance can become the latest event.

Remaining: publish/land through supported operations,
then retry completion. Installed v0.13.3 has no acceptance flag; installed
acceptance requires a published CLI containing the change. Jack Heart authorized
source delivery, not branch-binary writes to the installed Home.

Checks: `cargo test -p loopflow --lib historical_` (5), `completion` (29), and final `accepted_historical_uncertainty` (1) passed; formatting/architecture passed; `cargo clippy --all-targets -- -D warnings` passed; full matrix deferred to hosted CI.
