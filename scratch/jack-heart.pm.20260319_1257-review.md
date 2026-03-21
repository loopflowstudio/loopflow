# Branch review: jack-heart.pm.20260319_1257

## What was implemented

- Added a fuller PM integration surface in `lf` and `ops/pm.rs`: wave bootstrap/init, remote-wins pull, local-wins export, status reporting, ingest integration, and shared provider-role orchestration across Asana and Linear.
- Exposed PM sync as first-class built-ins with `import-pm`, `export-pm`, and the `pm-sync` flow, while keeping executor-managed three-way sync for PR-oriented runs.
- Extended the runtime and UI stack around wave execution: daemon journals, terminal-session persistence/routes, terminal-native workspaces in Concerto, keyboard routing, and multiplexer state/store coverage.
- Reorganized built-in flow/step surfaces around the newer model (`garden`, VSM governance flows, richer flow composition) and refreshed roadmap/docs to match the new operating model.

## Key choices

- **Directional PM commands stay explicit.** `pull` is the wave-level remote-wins refresh, `export` is the local-wins push, and `sync` remains the executor-facing three-way merge path.
- **Provider roles remain centralized.** Shared PM language and retry/update filtering live under `lfd/pm`, while `ops/pm.rs` owns wave-file orchestration and provider-ID writes.
- **Export is additive, not destructive.** Missing remote items are not recreated and remote-only items are not deleted/completed during export.
- **Step wrappers stay thin.** `import-pm`, `export-pm`, and `pm-sync` reuse the deterministic ops commands instead of introducing a second sync implementation.
- **Terminal workspace state is daemon-backed.** Terminal sessions, journals, and HTTP DTOs/routes carry the runtime state that Concerto renders in multiplexer/workspace views.

## How it fits together

`lfd` now owns more of the durable runtime state: journals, terminal sessions, richer wave events, and PM/provider orchestration. `lf` exposes that through new ops commands and built-in step/flow definitions, while Concerto consumes the new daemon surface to present terminal-native wave workspaces and attention state.

For PM specifically, the flow is: provider auth/config → shared provider adapters (`asana`, `linear`) → `ops/pm.rs` wave-file orchestration → optional step/flow wrappers (`import-pm`, `export-pm`, `pm-sync`) → executor lifecycle hooks for automated PR-oriented sync.

## Risks and bottlenecks

- This is a broad branch: Rust engine/runtime, PM sync, Python client, Swift app/tests, docs, and roadmap state all move together. Review by subsystem, not file order.
- Live PM behavior still depends on real provider config and hosted project semantics. Automated coverage is strong, but Asana/Linear round-trips still need a credentialed manual pass.
- PM ordering remains intentionally limited: rank-only changes are filtered, and export does not attempt destructive reconciliation.
- The local macOS `xcodebuild test` pass built the project and test bundles but did not terminate during this gate run, so the Swift package suite is the completed local Swift signal here.
- Concerto’s terminal workspace stack depends on macOS, GhosttyKit, and tmux behavior that unit tests can only approximate.

## What's not included

- Destructive PM export behavior such as remote deletes/completion for items missing locally.
- Cross-provider ordering parity beyond the current documented limitations.
- A replacement for executor-managed `pm_sync`; the executor still owns automated three-way sync at run boundaries.
- Full Notion parity beyond the early planning/docs groundwork in `wave/pm/`.

## Validation

- `cargo fmt --check` ✅
- `cargo clippy --all-targets --all-features -- -D warnings` ✅
- `cargo test --all` ✅
- `cargo test -p loopflow ops::pm::tests::pm_export_creates_updates_and_skips_without_recreating_missing_remote_items -- --exact` ✅
- `uv run pytest python/tests/` ✅ (`115 passed`)
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (`16 passed`)
- `swift test --package-path swift` ✅
- `cargo run --bin lf -- ops pm --help` ✅ (help text now advertises bootstrap/pull/export/sync)
- `cargo run --bin lf -- ops pm pull --help` ✅
- `cargo run --bin lf -- ops pm export --help` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ⏳ build completed, but the command did not terminate during this gate run

## Polish changes made during gate

- Updated the top-level PM CLI help string to advertise the shipped command surface (`bootstrap, pull, export, sync`).
- Corrected the PM roadmap item doc so `import-pm` is described as wrapping `lf ops pm pull`, not the team-level `pm import` command.
