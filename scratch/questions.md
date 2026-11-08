# LOO-810 implementation notes

Jack Heart, 2026-11-08: "ok, lets do C with simpler records"

Jack Heart: "lets pursue"

Build the design in [loo-810-server-contracts.md](loo-810-server-contracts.md):
immutable submitted creation requests survive Desktop restart. Client records
contain request ID, original destination, exact versioned payload and submission
time. The server owns outcomes. Retain server receipts in this version instead
of adding an expiry protocol. No editable outbox or generic command replay.

Carry forward one creation envelope for CLI/server and inline provider transition
explanations plus history. `started` covers history-less provider transitions.
These are implementation defaults, not separate quoted choices.

Budget constants remain provisional. Wave MEMORY.md is read-only from this Task.
No unresolved product choice prevents pursuing the design. Demo remains the next
interactive review; implementation direction does not authorize Auto-Ship.

Implementation choice: use 64 MiB per connection (two maximum-size producers)
and 256 MiB process-wide initially; 32 MiB would refuse ordinary paired reads
while either materializes. These count reservations and serialized output, not RSS.
