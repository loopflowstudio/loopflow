# Open questions / assumptions — W2-319

Headless run: proceeding on best judgment; these are the calls a reviewer might
reopen.

**Resolved by review ir_8095…** (kept for the record):
- Immutable-copy direction confirmed; content-addressed by byte digest.
- Lease (Active/Reserved) is the sole live-body authority; status may not veto.
- Promotion fence must also block reservation (shared/exclusive on the same
  lock); publish contract rewritten to staged ordered commits; `install`
  dispatches before home/journal/store-open; rollback revalidates and may refuse.

Still open / reviewable:

1. **Blanket live-body gate obstructiveness.** Any Active/Reserved lease blocks
   *all* global replacement. Given continuous dogfooding this will refuse often.
   **Assumption:** intended safety; the refusal names each body to drain a quiet
   window. The narrower rule (gate only frontier-advancing promotions) is a
   one-line relaxation but reopens the "swap under a running turn" window the
   incident hit — not taking it.

2. **Command surface name.** Chose `lf install {preflight,promote,rollback}` (new
   top-level `Commands::Install`). If `lf promote`/`lf self` is preferred it is a
   rename only.

3. **Reservation fence latency.** `LOCK_SH` is held only across the lease CAS via
   `spawn_blocking`, so steady-state contention is negligible; a promotion holding
   `LOCK_EX` briefly stalls new launches for the duration of its critical section
   (bounded by the migration/backup, seconds). Assumed acceptable; flagged in case
   the migration backup is large enough to want a launch-side timeout+retry.
