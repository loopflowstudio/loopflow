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

Budget constants remain provisional. No unresolved product choice prevents the
gate checks and demo. Implementation direction does not authorize Auto-Ship.

Implementation choice: use 64 MiB per connection (two maximum-size producers)
and 256 MiB process-wide initially; 32 MiB would refuse ordinary paired reads
while either materializes. These count reservations and serialized output, not RSS.

The grace-expiry test waits 61 seconds against the production deadline instead
of adding a test-only transport deadline. Gate owns this slower acceptance case.

Context check, 2026-11-08: the Graph's realign step trimmed Product memory from
16,417 to 15,924/16,000 tokens (75,277/131,072 bytes). Scratch is within both
limits. The earlier read-only note applied outside this memory-maintaining step;
no limit was raised. Product has no immediate child Wave directories here.

QA, 2026-11-08: use a disposable local repository and private Machine for creation
and a real shell for terminal recovery. The isolated Machine has no configured
provider account. Native builds ran, but computer use returned `cgWindowNotFound`
and the final Desktop state had no windows; physical sleep/wake, provider
continuation and mounted creation recovery remain for the native demo.

Jack Heart, 2026-11-09: "we cleared the codex limit"

Publication repair, 2026-11-09: ordinary concurrent Work loading reached the
two-producer cap. Desktop now delays request admission with five retries
(100–1600 ms) only after `busy {limit, max}` confirms nothing ran. This applies
the existing delay contract; it grants no replay after dispatch or disconnect.
