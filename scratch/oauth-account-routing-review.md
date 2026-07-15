# OAuth account routing review

Design: `scratch/oauth-account-routing.md`

## Verdict

Ready. Every done-when has implementation evidence and an automated proof. No
blocking or follow-up finding remains.

## Done-when audit

1. **Store semantics — met.** `provider_accounts_select_by_health_and_pin_sessions`
   proves same-provider accounts, utilization ordering, preferred tie-breaking,
   cooldown switching and expiry, and healthy session pins.
   `concurrent_provider_account_selection_reserves_distinct_accounts` uses two
   independent SQLite connections and proves the immediate transaction reserves
   different equal candidates.
2. **CLI — met.** `account_ids_are_shell_and_path_safe` proves slug validation;
   `format_account_shows_routing_state` proves visible health; CLI parser tests
   prove account login syntax and every control appears under `lf auth --help`.
   The built binary's `auth`, `connect`, and `disconnect` help were also inspected.
3. **Profiles — met.** Profile tests prove mode 0700, Claude/Codex native-home
   selection, shared symlinks, late skill relinking, independent auth locations,
   safe removal, and rejection of symlinked account homes.
4. **Harness routing — met.** Store selection proves a healthy pin resumes and a
   cooling pin returns a new-account selection without resume permission. Codex
   request tests prove `thread/resume` and `thread/start`. Claude and Codex frame
   tests prove warning and hard-limit parsing; route tests prove a hard frame's
   signal cools the active account.
5. **SSH lease — met.** SSH tests prove Claude and Codex managed accounts share
   the stdin preamble, managed access tokens are absent as plaintext, shell input
   stays quoted, and neither profile homes nor refresh-token fields cross the
   transport. The local CLIs validate each enabled profile first; a protocol
   test proves Codex requests `account/read` with `refreshToken: true`. The
   existing GitHub, PM, Doppler-secret, and exit-code behavior remains covered.
6. **Legacy behavior — met.** With no managed bundle/profile, the router returns
   no route and the existing vendor command/environment path remains in control.
   Existing engine, harness, auth, and empty-credential SSH tests all pass.
7. **README — met.** Examples cover connect/list/use/enable/disable/reset/disconnect,
   shared compiled skills, account pinning, `lf ssh`, process-tree inheritance,
   and the required local re-forward after Wave/tmux/host restart.
8. **Gates — met.** `cargo fmt --all -- --check`, all targeted account/auth/rate
   limit/SSH tests, `cargo test -p loopflow` (1,080 library tests plus every
   non-live integration suite passed; three CLI/live smokes ignored), and
   `cargo clippy -p loopflow --all-targets -- -D warnings` pass.

## Review findings fixed

- Selection originally serialized only callers sharing one in-process store.
  It now begins an immediate SQLite transaction, so separate Wave/Project/Task
  processes queue before reading and cannot reserve the same equal candidate.
- Account deletion originally trusted the profile path stored in SQLite. It now
  removes only the canonical provider/account path and refuses a mismatched or
  symlinked account home.
- Profiles now restore missing shared links on launch, covering skills compiled
  after the account was connected.
- Codex health now uses the most-consumed reported window, normalizes millisecond
  resets, and hard-limit cooldowns cannot land in the past because of clock skew.
- A forwarded lease with no managed account for one provider no longer selects a
  host-local managed profile and masks that provider's legacy forwarded token.
- SSH lease creation originally copied whatever access token an idle profile had
  on disk. It now validates Claude profiles and proactively refreshes Codex
  managed auth locally, then fails closed if any enabled account cannot produce
  a fresh access token. Refresh tokens still never cross SSH.
- The new migration's latest-version assertions were advanced to
  `0.11.004_provider_accounts` after the full suite caught the stale expectations.

## Deliberate boundary

No real OAuth login, paid provider turn, or live SSH host was used in the review.
Those would mutate external accounts or spend tokens. Installed Claude and Codex
help confirmed the invoked login surfaces; process construction, protocol frames,
credential isolation, and transport behavior are covered locally. As designed,
a hard limit changes accounts at the next provider-session launch rather than
moving an in-flight vendor conversation between identities.
