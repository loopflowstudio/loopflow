# LOO-694 fix forward: owner that never answers (after #1788, f6af59cc66)

Jack's verify on main found that #1788 did not open the real MTG-214 Session `7d29bee8…`. Prepare still
reported `active` with only Move here. Its owner `2b6fdd25…` (pid 20629, a pre-upgrade `session serve --terminal`)
is alive, but it never answers `{"kind":"controlled"}`. `viewed_summary` kept the stale `viewed=1`, and Move
here ran `transport::attach` on the same unresponsive socket with no fallback.
Do not kill the live MTG-214 chain to reproduce this.

## Done (2026-10-30)

- `human_session::open` now probes a running owner with `transport::controlled` (1 s timeout) before attaching.
  If the owner doesn't answer, open skips the attach and stops the provider. The same provider thread
  (`17e4176a…`) then resumes in a new owner, with no input replayed.
- `viewed_summary` treats an owner that doesn't answer as unviewed, so prepare reads `detached` and offers Reconnect.
- The test `opening_takes_over_from_an_owner_that_never_answers` uses an owner that accepts connections but never
  replies. It fails on main and passes here. Fixtures that attach silently now answer the probe, so they still
  cover an attach that is never acknowledged.
- Ran the branch build against the real MTG-214 Session, prepare only: `detached`, `[Reconnect]`. Resume was not run
  because it would stop the live provider.

## Remaining

- Publish as a new PR. The tree diff against origin/main contains only this fix. The commit list still carries
  #1788's pre-squash commits, so squash on publish.
- Demo/review: in Desktop, Reconnect takes over MTG-214 when Jack chooses to.

Check: `cargo test -p loopflow --test transport_tests -- opening_ attachment_ stops_reading` passed 5 tests; clippy clean (implement step).

## Concept review (2026-10-30)

No change. Jack's 2026-10-26 decision that `lf resume` takes over from whatever holds the conversation already
covers an owner that never answers: Reconnect continues the same provider thread, so the name stays truthful. The
old serve stops blocking because the Session's owner becomes the new one. The 1 s stall threshold follows from that
decision; `scratch/questions.md` records it, and it isn't an open product choice. The two unknowns stay distinct on
purpose. An owner that answers and then fails to attach may still hold a live viewer, so only the silent owner is
taken over.
