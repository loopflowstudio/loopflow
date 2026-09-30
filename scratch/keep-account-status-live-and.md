# Live account handling · LOO-340

Jack Heart requested current account status, one machine account authority,
reliable browser targeting, consistent identity checks, launch-wide account
selection, and explicit banked reset redemption. LOO-339's identity core is the
base. Jack's later steer removes per-step account choice. Never spend a real
reset during verification; use fake credentials and disposable Homes throughout.

## Complete target

- `lf auth status [provider]` refreshes managed identity and usage before reporting.
  `--cached` retains read-only inspection without provider or broker contact.
  Expired windows remain dated observations, never evidence of current fullness.
  Failed refresh preserves observations and distinguishes unavailable from rejected.
- Account identities, browser bindings, routing, credentials, usage and reset
  credits have one machine owner shared across Homes. Move existing consumers
  and migrate retained state without parallel live copies. Home-local Task
  resolution remains LOO-334's responsibility; do not move Work storage here.
- Managed Codex connect delegates browser opening only to the saved Chrome
  profile; suppress the provider's independent browser open.
- Cached Claude inspection and routing enforce the same expected-email,
  observed-user and duplicate-login rules as Codex, tied to current credentials.
  Stale `.claude.json` cannot establish identity.
- One launch can name one account per provider. Capture that selection with the
  invocation and propagate it into every child, including resumed and human
  steps. No account overrides on individual steps. Explicit selection and
  Session affinity retain their existing precedence and failure semantics.
- Read Codex `rateLimitResetCredits` count, status and expiry. A named-account
  redemption command alone can spend one, and reports before/after windows.
  Status, routing and tests never redeem automatically.

## Existing owners and call paths

`lf::AuthCommand::Status` dispatches through `commands::auth` to `auth_status`.
`managed_rows` already locks each managed login, calls `subscription::poll_account`,
validates identity, and writes accepted observations through the store. Rendering
already hides expired percentages. JSON windows are dated historical rows;
`verified_windows` identifies observations returned by the current invocation.
`provider_account::{open_account_store,read_account_store}` currently select the
Home database. The routing owner also reads these accounts and windows. Change
that owner when moving storage, rather than introduce another catalog.

## This slice

Move Codex connect from the native `login` command to the existing app-server
auth transport. `account/login/start` returns the callback URL with provider
browser opening disabled; Loopflow opens it through the saved Chrome profile.
Keep staged credentials and identity validation. The existing auth handle owns
the child through the matching `account/login/completed` notification, failure,
or cancellation. Reuse the same app-server connection for token refresh.

Done when simulated app-server proofs cover a matching successful completion,
unrelated completion, failure, EOF, timeout and cancellation; managed connect
proofs retain saved-profile selection, staged reconnect and identity rejection.
These are synthetic provider/browser proofs, not real OAuth or desktop acceptance.
No real account or reset is used.

## Remaining slices

Machine store migration and LOO-334 ownership coordination;
Claude cached identity and routing; captured per-provider launch selection and a
mixed-provider multi-step Flow proof; reset-credit display and explicit redemption
using a verified provider protocol. Each must replace its existing consumer in
the same cut. The complete Task is unfinished until all outcomes have evidence.

## Slice ledger

### Pass 1 · live status

Switched the public status consumer to the existing verified observation path;
removed the opt-in `--verify` flag and migrated offline callers to `--cached`.
No new store, polling implementation or DTO was introduced. Repository searches
found no remaining current `auth status --verify` callers under source, docs,
Python or tests (historical Wave memory remains historical).

Comparison: `6d4727d33f1101d5cc7e01143878f95c8db1ccd2` to working tree.
Non-test Rust: +6 / −5 lines. Documentation and builtin guidance: +19 / −18.
Excluded: integration tests (+72 / −28), scratch design/assumptions, generated
files (none changed). Counts use `git diff --numstat` over named changed files.

Proof uses an environment with inherited `LF_*` removed and `LF_BIN` pinned to
this checkout's compiled CLI, plus the tests' disposable Homes and fake provider:

- `cargo test -p loopflow --test auth_tests -- --test-threads=1`: 7 passed.
  Default status replaces expired 100%-used observations with 12% used / 88%
  left. Unavailable and omitted responses leave usage unknown and retain the
  original rows. Cached inspection retains missingness and performs no broker
  request; identity and rejection preservation regressions remain passing.
- `uv run pytest python/tests/test_loopflow_skill_alignment.py -q`: 4 passed.
- `cargo test -p loopflow --lib auth_report_fixture_preserves_missingness_and_readable_widths`:
  1 passed; JSON round trip and readable 80/120-column output retain missingness.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, and
  `git diff --check`: passed.

Review found that changing only dispatch would leave builtin instructions
promising offline behavior and invoking the removed flag. Those consumers and
the getting-started guide now match the CLI. JSON still carries dated observations,
so consumers must check reset time; text never displays an expired percentage.
Provider requests retain existing bounded timeouts. No live account, reset
redemption, installed acceptance, full gate or publication was attempted.

The resource preflight passed with 50 GiB free after reclaiming an inactive
allowlisted build. The busy uv cache was retained; verification stayed above the
32 GiB floor. Remaining target slices above are unchanged and not proven.

Boundary completion on 2026-09-30: all eight implementation, test and guidance
files match the supplied workspace SHA-256 values. The recorded behavioral
proofs above were retained without rerunning unchanged behavior. The additional
required `cargo test -p loopflow --lib engine::builtins::tests` passed all 14
tests with inherited `LF_*` removed and the source CLI pinned. Fresh
`cargo fmt --check`, all-target Clippy and diff checks passed; resource preflight
reported 51 GiB free. Review confirmed that expired windows still render unknown,
unavailable verification retains dated observations, and cached reads keep their
read-only store path. No additional production changes were needed. This closes
only the marked live-status slice; the complete Task remains unfinished.

Compression: paired failure setup with its diagnostic in the status regression,
removed repeated output decoding, an unnecessary window clone and a redundant
assertion. The isolated public-CLI status regression passed (1 test); formatting,
all-target Clippy and diff checks passed. Production behavior is unchanged;
earlier untouched proofs retain their scope. No real accounts or resets were used.


### Pass 2 · Codex browser ownership

Switched `CodexAuthBroker::start_auth` from `codex login` plus `BROWSER=echo`
to `codex app-server` and `account/login/start`. OpenAI's account processor
sets `open_browser: false`; Loopflow's existing saved-profile opener consumes
`authUrl`. The returned `loginId` selects completion. Staged installation still
requires successful completion and the existing live identity validation.

The private auth connection owns the app-server process group and pipes until
completion, error, timeout or handle cancellation. Token refresh uses this same
connection; the predecessor spawn/write/read helpers and native login launch
are deleted. Error messages omit provider payloads and authorization query data.
The existing account store and browser-binding authority are unchanged.

Comparison: `dcbe062e2cbd32fdd8ee6728da325543efd7eca4` to this working tree.
Non-test Rust: +190 / −155 lines, including the new module before its test block
and the production portion of `provider_auth/mod.rs`. Documentation: +9 / −4.
Excluded: Rust test blocks and fixture edits, scratch, generated files (none).
Counts split production at the test-module boundary and compare lines with
`difflib.ndiff`; the new file is included even while untracked.

Executed with inherited `LF_*` removed and `LF_BIN` pinned to this checkout's
compiled CLI; providers, browsers, credentials and account Homes are fixtures:

- `cargo test -p loopflow --lib provider_auth::codex::tests -- --test-threads=1`:
  3 passed. Matching success, unrelated notifications, provider failure, EOF,
  malformed output, secret-safe errors, cancellation and simulated ten-minute
  timeout. Cancellation/timeout assert actual fixture-child termination.
- `cargo test -p loopflow --lib lf::commands::auth::account_first_tests -- --test-threads=1`:
  12 passed. Saved Chrome selection, mismatch refusal and staged preservation
  remain covered. A failed completion after staging credential bytes preserves
  the existing account and credential. The fake provider refuses native login.
- `cargo test -p loopflow --lib provider_auth::tests::codex_refresh -- --test-threads=1`:
  2 passed; managed refresh and missing-provider behavior remain intact.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, and
  `git diff --check`: passed. Resource preflight passed with 50.4 GiB free.

Review removed the leftover refresh forwarding wrapper so both auth consumers
call the single transport directly. The dedicated connection keeps cancellation
ownership visible; no generic RPC registry or second auth flow was added.
Protocol source was inspected on 2026-09-30:
https://github.com/openai/codex/blob/main/codex-rs/app-server/src/request_processors/account_processor.rs
This establishes source behavior, not an installed-provider or real browser demo.
No live account, real reset, publication, full gate or Task completion is claimed.
This closes the marked browser-ownership slice. The remaining slices above
still require implementation and proof.
