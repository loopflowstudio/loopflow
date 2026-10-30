# Verify LOO-694 on main (f6af59cc66) — fix forward

- `cargo test -p loopflow --test transport_tests opening_reattaches` on main's tree: pass.
- Real MTG-214 Session `7d29bee8…`, read-only, with main's `target/debug/lf session resume <id> --prepare --json`:
  still `state: active`, actions Open here (unavailable) + Move here. Same as installed 0.15.1. PR #1788's "Try it" claim doesn't hold here.
- Why: transport owner `2b6fdd25…` (pid 20629, `lf session serve --terminal`, pre-upgrade build, `transport::idle` thread) is alive.
  Its main thread loops in `transport::serve` (sleep), but a fresh connection to its socket gets no reply to `{"kind":"controlled"}` within 8 s.
  `controlled()` errors, so `viewed_summary` keeps the stale `viewed=1`.
- Move here runs `transport::attach(&owner, …)` on that same deaf socket, with no fallback. So the Session is still unopenable.
- The new test uses a current-build owner with a hand-staled row. It never puts an earlier-build owner across a version change.

Gap to repair: when the owner is alive but doesn't answer its socket, opening must take the conversation over on the same provider thread.
Prove it with a fixture owner that accepts but never replies.
Don't kill the live MTG-214 chain to reproduce.

## Fix forward (2026-10-30)

- `human_session::open` probes a running owner (`transport::controlled`, 1 s) before attaching; no answer → skip attach, delegate, stop the provider, resume the same thread. `viewed_summary` treats a non-answering owner as unviewed, so prepare reads `detached` / Reconnect.
- Test `opening_takes_over_from_an_owner_that_never_answers`: fails on main (state `active`, Move here), passes with the fix. Silent-attach fixtures now answer the probe so their unacknowledged-attach coverage stays.
- Real MTG-214, branch build, prepare only: `detached`, actions `[Reconnect]`, no terminal ids. Resume not run (would stop its live provider); Reconnect in Desktop takes it over.
- `cargo test -p loopflow --test transport_tests -- opening_ attachment_ stops_reading`: 5 pass; clippy clean.
- Compress: code already minimal; README paragraph reflowed (ef6da186a3). Branch still carries #1788's unsquashed commits; rebase onto origin/main before opening the new PR (owner: publish).
