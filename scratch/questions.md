# Open questions and assumptions — W2-311

Headless run: recorded and proceeded rather than blocking.

## Assumptions made

1. **`lf doctor` may perform one best-effort single-ref `git fetch origin main`.**
   Comparing against a stale local `origin/main` would silently under-report the
   gap, which is the defect itself. A fetch touches only remote-tracking refs, so
   it is safe beside live bodies (MEMORY's concurrency hazards are rebase/reset).
   Assumed acceptable in a human-invoked diagnostic; on failure the check falls
   back to the local ref and names the caveat rather than staying quiet.

2. **`Unprovable` warns rather than staying silent**, so `lf doctor` outside a
   loopflow checkout emits one honest warn line. Chosen over a special case that
   keeps quiet outside a checkout: the whole point is that silence must never
   mean "fine". Warn (not Fail) keeps `doctor`'s cron exit code intact.

3. **`OffMain` (a dev build off a feature branch) is `Ok`, not a warning.**
   Warning would fire on every worktree; the measured cost is the release fleet.
   Recorded as an open question in the design rather than designed in.

4. **`source_revision` on `BinaryProvenance` is `Option<String>`.** Required
   would break deserialization of every historical generation row. Matches the
   precedent one field up (`provenance: Option<BinaryProvenance>`). Not a wire
   DTO (Rust-only; no Swift mirror, no `tests/fixtures/dto` entry), so the
   no-defaults DTO rule does not apply.

5. **`lf status` is out of scope.** The seed calls it defensible, not required;
   one supported read satisfies the Done-when.

## Genuinely open

- Should a dev build report merged commits since its merge-base? Deferred until a
  stale dev build causes a measured incident.
- The `fleet-freshness` check reports every currently-live body as unstamped
  until the fleet rebuilds, since `source_revision` is new. That is the true
  answer, not a gap — but it means the check has no non-trivial output on this
  host until an operator rebuilds. Accepted: the `binary-freshness` self-check
  carries the demo today.

## Verification risk (not an assumption — a known hazard)

Local `cargo` verification may be impossible on this host: syspolicyd kills newly
linked test binaries before `main` (filed as 47880291), and the cargo build lock
is shared across worktrees, so a run that looks hung is often waiting on another
Task's process. If verification is blocked at implementation time, the correct
report is "the proof is unavailable", not a narrated expected result. Never
`pkill` cargo by pattern (it would kill a sibling Task's live run); match
`rustup/toolchains` to find real cargo processes, since `grep cargo` also matches
agent bodies carrying the word in their prompt.
