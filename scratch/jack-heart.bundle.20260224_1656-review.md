# Review: Concerto bundled lfd/lf runtime

## What was implemented

- Added bundled daemon runtime support in Concerto:
  - `BundledDaemonManager` starts `lfd` from the app bundle with per-run token/port and per-repo sqlite path.
  - `BundledDaemonRegistry` shares one bundled daemon per canonical repo path with refcounted acquire/release.
  - `RepoState` now connects in two modes (`bundled` default, `remote`) and performs handshake via a shared connection path.
- Reworked connection persistence:
  - `ConnectionStore` now persists connection mode + remote config only.
  - Bundled runtime details (ephemeral port/token) are not persisted.
- Updated connection UX:
  - Connection settings now show **Bundled** and **Remote** modes.
  - Added CLI install/uninstall controls through `CLIInstallManager`.
- Build/release integration:
  - `scripts/dev.py` now copies built `lf` and `lfd` into app bundle for dev and release installs.
- Rust daemon fix:
  - `LFD_DB_PATH` now accepts absolute paths (plus existing relative-path behavior).
- Additional polish in this gate pass:
  - Registry now force-stops bundled daemons on app termination notification.
  - CLI installer now refuses overwriting non-symlink binaries and only uninstalls managed symlinks.
  - `RepoWindow` repo-open logic now tracks canonical repo path instead of one-shot boolean guard.

## Key choices

- **Bundled as default mode**: Users do not need preinstalled local daemon setup; remote mode remains opt-in.
- **Ephemeral runtime secrets**: token/port are generated per run and intentionally not stored.
- **Repo-scoped daemon sharing**: canonicalized repo path + registry refcount avoids duplicate daemons across windows.
- **Absolute sqlite path support in lfd**: cleanly enables app-support DB placement from Concerto without symlink hacks.
- **CLI install safety guardrails**: avoid clobbering existing user binaries in selected install directories.

## How it fits together

Concerto opens a repo, resolves connection mode from `ConnectionStore`, and either acquires a bundled `lfd` process from `BundledDaemonRegistry` or uses persisted remote settings. `RepoState` runs the same handshake pipeline (TLS/auth/repo discovery/ws probe) for both modes, then wires `WaveService` + `EventService` with mode-appropriate connection/token values. Build scripts now package `lfd`/`lf` into the app, and Rust `lfd` accepts the absolute DB path Concerto provides.

## Risks and bottlenecks

- Bundled daemon startup still has an ephemeral-port race window (accepted in design).
- UI tests are environment-sensitive in headless/automation-constrained contexts (see test results below).
- Bundled daemon shutdown now has both per-window release and app-termination safety net; lifecycle complexity is higher than prior single local-mode assumption.

## What's not included

- No universal-binary packaging enforcement for `lf`/`lfd` in `scripts/dev.py` (still builds host-arch binaries).
- No additional launchd/LaunchAgent compatibility path for bundled mode (intentionally process-lifetime owned by app).
- No extra migration logic beyond existing legacy connection key handling.

## Validation run

- ✅ `cargo fmt --all -- --check`
- ✅ `cargo clippy --all-targets -- -D warnings`
- ✅ `cargo test --all` (docker-dependent tests skipped in environments without `/var/run/docker.sock`)
- ✅ `uv run pytest python/tests/` — 47 passed
- ✅ `tests/e2e/test_smoke.sh`
- ✅ `swift test --package-path swift` — 130 tests in 22 suites passed (includes `ConnectionStoreTests`, `CLIInstallManagerTests`)
