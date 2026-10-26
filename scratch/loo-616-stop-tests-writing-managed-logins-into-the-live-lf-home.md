# LOO-616: Stop tests writing managed logins into the live lf home

Status: implemented 2026-10-26; gate remains. LOO-602 found the symptom.

## Problem

`lf account --cached` on Jack Heart's Machine lists
`codex fixture-cf433b35f6f1 · fixture@example.com` (routing explicit-only). A
test credential reached the real `~/.codex`. Real `lf` then filed it as a
managed login. The leak also overwrote the live Codex login that every
shared-mode agent was using at the time.

## Findings (evidence, 2026-10-26)

- **The bytes come from one helper.** The stray `auth.json` holds
  `access_token: "fixture"` and claims `{"email":"fixture@example.com","sub":"fixture"}`.
  Only `managed_codex_account` in `rust/loopflow/tests/session_lifecycle_tests.rs`
  writes that exact credential. Other helpers use `synthetic-fixture-token`,
  `fixture-only` or other emails.
- **Real lf filed it.** The `provider_account_switches` table in `~/.lf/loopflow.db`
  shows real switches to `jack-42d1021d3f2d` at `2026-10-14 02:05:01` UTC, which
  matches the row's `created_at` exactly, and again at `2026-10-15 02:10:55`,
  which matches the stray `auth.json` mtime. Both times `activation::save_back`
  found the fixture credential in the native `~/.codex`. No stored account
  owned it, so `save_back` created `account_id_for_login(fixture@example.com)`
  marked `ExplicitOnly`. The fixture reached `~/.codex` at least twice.
  There have been no further leaks since 2026-10-15.
- **Cause: an inherited `CODEX_HOME`.** Codex tool shells export
  `CODEX_HOME=/Users/jack/.codex`, observed live in the environment of Codex
  child processes. The session_lifecycle `Fixture::command` sets `HOME`,
  `LF_HOME` and `PATH`, and scrubs `LF_*`, `LOOPFLOW_*`, `LINEAR_*` and
  `CMUX_*`. It leaves `CODEX_HOME` and `CLAUDE_CONFIG_DIR` in place.
  `headless_history_is_discoverable_without_entering_the_interactive_list`
  registers the fixture account and runs a shared-mode codex launch.
  `RouteSelection::launch_as` → `activation::native_home` prefers the inherited
  `CODEX_HOME` over the fixture's `HOME`. `is_account_home` filters only the
  *fixture's* `LF_HOME/accounts`. `activate` therefore saves the developer's real
  native login into the throwaway store, which is then deleted, and writes the
  fixture login into `/Users/jack/.codex/auth.json`.
- **The class is wide.** `support::EnvGuard` already pins both provider homes
  under its temp Machine (#1416), but about 50 test files spawn `lf` directly
  and do not mention `CODEX_HOME`. Any fixture with a stored provider account
  and a shared launch repeats the bug when `cargo test` runs from a Codex
  agent, or from a parent lf launch that exported an isolated account home.
  In that case the fixture overwrites a real *stored profile* instead of
  `~/.codex`.
- **Coverage.** `tests/support/mod.rs` is compiled into every support-using
  integration binary and into the library's unit tests (`src/lib.rs`,
  `#[path = "../tests/support/mod.rs"] mod test_support`). Three standalone
  binaries spawn `lf` without `mod support;`: `one_machine_tests`,
  `process_ownership_tests` and `two_people`.
- **Stray state is small.** It is one `provider_accounts` row, the home
  `~/.lf/accounts/codex/fixture-cf433b35f6f1/` and the lock file
  `~/.lf/accounts/codex/.fixture-cf433b35f6f1.login.lock`. No limits, switches,
  routes, session pins or browser bindings reference it.
  `lf account disconnect` keeps the row as `Missing`, and there is no delete
  command.
- **Live logins look intact.** `~/.codex` is currently `manabot-eng`, and every
  real account reports cached-connected. The native credentials overwritten on
  10-14 and 10-15 had profile copies, so there is no evident loss.

## Demo

```bash
sentinel=$(mktemp -d)/codex; claude=$(mktemp -d)/claude
CODEX_HOME=$sentinel CLAUDE_CONFIG_DIR=$claude \
  cargo test -p loopflow --test session_lifecycle_tests \
  headless_history_is_discoverable_without_entering_the_interactive_list < /dev/null
test ! -e "$sentinel" && test ! -e "$claude" && echo untouched
lf account --cached | grep -c fixture    # 0
```

Before the fix, `$sentinel/auth.json` appears and holds `fixture@example.com`.
After the fix, the test passes and both paths stay absent.

## Approach

**Test processes never carry the developer's provider homes.** One pre-main
initializer in `rust/loopflow/tests/support/mod.rs` removes `CODEX_HOME` and
`CLAUDE_CONFIG_DIR` from the test process before any test thread starts. Every
`lf` child then derives its native home from the fixture's `HOME` or from a
home the fixture sets explicitly. That is already how fixtures that care
(`session_launch_contract`, `context_launch_tests`, `EnvGuard`) behave. The fix
lives in one place and covers the integration binary, standalone binaries,
and library unit tests.

- Dependency: add `ctor` as a dev-dependency of `loopflow` and use
  `#[ctor::ctor]` (with the `unsafe` marker the current release requires) on a
  `fn` that calls `std::env::remove_var` for both names. A comment explains why:
  agent shells such as Codex export the developer's real provider home. Running
  before `main` means no other thread exists yet. Library tests reach it through
  `test_support`.
  Decision: use `ctor`, not a hand-written `link_section`, because it is the
  standard, reviewed pre-main mechanism. If offline builds turn out to block the
  new dependency, fall back to a hand-written link section with a
  `// SAFETY:` comment.
- Add `mod support;` to `one_machine_tests.rs`, `process_ownership_tests.rs`
  and `two_people.rs`, so every `lf`-spawning binary links the initializer.
  Mark `support` with `#[allow(dead_code)]` where it is otherwise unused.
- Leave `EnvGuard`'s explicit `CODEX_HOME`/`CLAUDE_CONFIG_DIR` pins in place.
  They give fixtures without a `HOME` override a private native home, which
  removal alone does not. Its drop now removes both instead of restoring a
  previous value, which the scrub guarantees is absent. Do not add per-fixture `env_remove` lines; the
  initializer replaces that need.
- Value test in `test_environment_tests.rs`. The parent re-execs the current
  test binary (`std::env::current_exe()`, `--exact <child> --nocapture`) with
  `CODEX_HOME`/`CLAUDE_CONFIG_DIR` set to sentinel temp paths and a marker
  variable. The child test is a no-op without the marker; with it, it asserts
  that both variables are absent. This proves the scrub without depending on
  the runner's environment, and it passes in CI.
- Remove the stray login on Jack Heart's Machine. Run
  `lf account disconnect codex fixture@example.com`, which removes the home
  through the product path; it runs `codex logout` against the fixture home
  only. Then
  `sqlite3 ~/.lf/loopflow.db "DELETE FROM provider_accounts WHERE provider='codex' AND account_id='fixture-cf433b35f6f1'"`
  and remove `~/.lf/accounts/codex/.fixture-cf433b35f6f1.login.lock`. Check
  first that no running `lf` holds that lock. This is a one-time machine
  repair, not code.

## Alternatives considered

- **Per-fixture `env_remove("CODEX_HOME")`.** This needs about 50 edits, and
  every new fixture must remember it. That is exactly the drift that produced
  this bug. Rejected.
- **Product change: treat any inherited home outside this Machine's
  `accounts/` as foreign.** Production correctly honors `CODEX_HOME=~/.codex`
  in shared mode, and data-clone/world recipes set it on purpose. Rejected:
  the defect is in the test environment.
- **`.cargo/config.toml` `[env]` with `force = true`.** Cargo can only set
  variables, not unset them. It would also redirect `cargo run --bin lf`
  during development. Rejected.
- **Nextest setup script.** It can set but not unset variables, and it does
  not cover plain `cargo test`. Rejected.

## Contract

- **User-visible outcome:** running any Rust test from a Codex or lf agent shell
  leaves `~/.codex`, `~/.claude`, the real stored profiles and `~/.lf` untouched.
  `lf account --cached` on Jack Heart's Machine no longer lists the fixture login.
- **Source of truth:** the test process environment at start. `lf` children
  inherit it. Fixtures that need a provider home set it explicitly.
- **Affected surfaces:** `rust/loopflow/Cargo.toml` (dev-dependency),
  `tests/support/mod.rs` (initializer; `EnvGuard` drop), four standalone test
  binaries (`one_machine_tests`, `process_ownership_tests`, `two_people`,
  `discovery_tests`, whose own provider-home save/restore is deleted),
  `linear/graph_tests.rs` (redundant `env_remove` deleted),
  `test_environment_tests.rs`, and TESTING.md's ambient-environment paragraph,
  which names `codex_live_smoke` as the one unlinked binary. Production code
  does not change.
- **Exclusions:** no new `lf account delete` command; disconnect + SQL is enough
  for this one-time cleanup. Codex's own `CODEX_HOME` export is not changed.
  Other `LF_*` ambient scrubbing (`AMBIENT_TASK_ENV`) is unchanged.
- **Forbidden outcomes:** per-fixture scrub lists alongside the initializer; a
  second guard type; product code that inspects test markers.

## Done when

- The demo command passes and both sentinel paths stay absent (gate). Record
  the before-fix run of the same command, which creates `$sentinel/auth.json`,
  as a one-line check result.
- `cargo test -p loopflow --test test_environment_tests` (standalone binary) passes,
  including the re-exec scrub test.
- `cargo test -p loopflow --test one_machine_tests --test process_ownership_tests --test two_people --no-run`
  builds. `cargo clippy --all-targets -- -D warnings` passes.
- `lf account --cached | grep -c fixture` prints `0` on Jack Heart's Machine.
  `~/.lf/accounts/codex/` contains no `fixture-*` entries.

## Implementation notes (2026-10-26)

- `ctor` 1.0.13 resolved; `#[ctor::ctor(unsafe)]` is the required marker.
- Also linked `support` into `discovery_tests` and deleted its own
  provider-home save/remove/restore in `HomeGuard`, and dropped the redundant
  `.env_remove("CLAUDE_CONFIG_DIR")` in `linear/graph_tests.rs`. `transport_tests`
  already linked support. `codex_live_smoke` stays unlinked: it targets the
  real provider deliberately.
- Machine repair done: disconnect left native `~/.codex/auth.json` byte-identical;
  row deleted, lock removed; `lf account --cached | grep -c fixture` → 0.

## Check results

- Realign 2026-10-26: every `[[test]]` binary but `codex_live_smoke` links support; no upstream changes since base; TESTING.md corrected to name the exception. No rerun (prose only).
- Before fix: demo test passed and created `$sentinel/auth.json`. After fix: passed, both sentinels absent. `test_environment_tests` 3/3 pass; `discovery_tests` 13/13; standalone `--no-run` builds; `cargo clippy --all-targets -D warnings` clean. Compress: EnvGuard restore simplified; `test_environment_tests` 3/3, `discovery_tests` 13/13, clippy clean. Gate owns affected suites.
- Concept review 2026-10-26: the contract (a test process starts without the developer's provider homes; fixtures that need one set it) holds. No test or library path reads the runner's `CODEX_HOME`/`CLAUDE_CONFIG_DIR` on purpose, and only `codex_live_smoke` targets a real provider. No design change and no open choice. No rerun.
