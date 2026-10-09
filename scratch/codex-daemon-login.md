# Codex daemon adopts the selected login

## Outcome and accepted decision (2026-10-09)

After `lf account codex use`, a bare `codex` should use the selected login,
including when `auth.json` already holds it but the daemon does not.

Jack chose a bounded wait after an initial repair interrupted a running turn:
install the login first, release the credential lock so lf launches can proceed,
then give daemon turns up to five minutes before restarting. Expiry permits
interruption; abandoning the restart at expiry is not the accepted behavior.

## Gate implementation and remaining review

- `use_account` installs the account, releases its credential lock, then reconciles
  the daemon even when the file already held the selected login.
- Missing/refused sockets mean no daemon. Handshake, protocol, timeout and missing
  email failures are errors, not silent absence. Gate chose an initial-probe error
  without restart; the error explicitly retains the installed login and names the
  retry command. This reversible choice is recorded in `scratch/questions.md`, not
  represented as Jack's approval.
- Unknown activity keeps the grace period. Loaded-thread pagination is followed;
  only readable idle state (or daemon absence) ends the wait early. A single
  five-minute timeout includes polling and probes. Expiry still restarts and warns
  that remaining turns may be interrupted, without claiming any turn was cut.
- Login/activity probes each have a five-second timeout. Restart now has a
  thirty-second timeout and kills its owned command on cancellation; a post-restart
  login check must match. Cancelling during the grace period retains the installed
  login; repeating `use` reconciles it again.
- lf terminals explicitly pass `--no-daemon`; headless app-server ownership is
  unchanged. `docs/subscriptions.md` covers the behavior and errors.
- Automated acceptance uses disposable sockets and a fake restart executable. It
  exercises the actual reconciliation helper, not public account activation or a
  configured Codex daemon. `TESTING.md` records the explicit real-time expiry check.
- **Demo/review remains:** real configured Codex handshake, busy wait/restart and
  terminal experience in an isolated setup, never Jack's active daemon. The gate
  choice of error rather than warning on unreadable login remains reviewable.
- **Delivery remains:** refresh PR copy at publication. No publication-state check
  or publication occurred at gate. `scratch/pr-review.html` is marked historical
  at `74e7dae93`; its abandoned-wait alternative is no longer presented as open.

## Evidence and limits (2026-10-09)

- Installed lf 0.13.10 changed `auth.json` while the daemon retained its old login
  and pid. At `8c1838a4f`, a disposable `LF_HOME` replay switched the real daemon
  both ways and verified the selected email under a new pid; repeating the same
  selection kept the pid. That pre-wait replay interrupted a running turn.
- A bare `codex resume` used the control socket; observed lf/cmux terminals with
  configuration overrides did not. Configuration overrides explaining this is
  an inference from a binary diagnostic, not a source-confirmed contract.
- The observed daemon retained its startup login after a disk change. The
  inspected protocol schema offered no disk-login reload call. This supports
  restarting for this repair, not a universal claim about every refresh path.
- codex-cli 0.161.0 accepted `--no-daemon` before `resume`, `fork` and `exec`.
  Older versions without the flag reject these launches; no compatibility shim
  is planned.
- Automatic launch switches (`SwitchCause::Exhaustion`) still leave the daemon
  alone. A bare Codex can lag after one; extending daemon reconciliation to
  automatic switches is outside this change.
- Whether a stale daemon can overwrite `auth.json` with old refreshed tokens
  remains unverified; it was not observed during about an hour of inspection.
- Upstream through merged `3e1e6245c` changes owned-engine cleanup, not this shared
  daemon's login reconciliation. No Wave or Task is bound in `lf context`;
  no Wave memory was selected or curated.

Check (gate 2026-10-09): `cargo build -p loopflow --bin lf --jobs 4` passed; network-isolated `cargo nextest run -p loopflow --lib --test session_cli_tests --test agent_startup_tests --run-ignored all -E 'test(provider_auth::codex::) | test(lf::commands::account::) | binary(session_cli_tests) | binary(agent_startup_tests)'` passed 56, including expiry at 300.177s; `cargo nextest run -p loopflow --lib -E 'test(provider_account::) | test(busy_and_unreadable_turns_finish_before_restart)'` passed 42 after strengthening the busy-to-idle completion assertion; `cargo test -p loopflow --test documented_commands` passed 3 (100 distinct Rust tests, build/tests on 1.98.1); `cargo +stable fmt --all -- --check` and `cargo +stable clippy --all-targets --jobs 4 -- -D warnings` passed on current stable 1.99.0; `uv run python scripts/check_architecture.py`, `cd website && uv run python dev.py test -k test_docs_subscriptions_page_owns_account_selection`, and `git diff --check` passed. Test runs cleared inherited LF/LOOPFLOW authority, pinned the source CLI and used disposable homes; CI owns the full materialized matrix, configured Codex experience remains demo/review-owned.
