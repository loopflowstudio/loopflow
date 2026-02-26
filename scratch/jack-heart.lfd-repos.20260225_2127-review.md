# Review: Live Auth Contract Harness + iOS Action Rail

Branch: `jack-heart.lfd-repos.20260225_2127`

## What was implemented

Two independent deliverables on one branch:

1. **Live auth contract validation harness** (`scripts/test_auth_live_contract.py`) — an end-to-end test that starts lfd, exercises the `/v0/auth/{provider}` HTTP + WebSocket flow for GitHub/Claude/Codex, and captures machine-readable evidence (CLI transcripts, status samples, auth events, credential tree snapshots). Accompanied by Rust hardening in `provider_auth.rs`: broader user-code parsing, ANSI escape stripping, GitHub credential-file fallback, logout idempotency, and `claude auth login` command update.

2. **iOS suggested action rail** — wires `ActionButtonsView` into `MobileWaveDetailView` via a bottom `.safeAreaInset`, with session lifecycle ownership moved from `WaveSessionView` to the iOS detail container. `WaveSessionView` gains `showsSuggestedActions` and `managesLifecycle` parameters (defaulting `true` for backwards compatibility) so iOS can disable duplicate rendering while macOS behavior is unchanged.

## Key choices

**Evidence-based validation over unit mocks.** The auth harness runs real CLI binaries against a real lfd instance in an isolated `$HOME`. This catches upstream CLI drift (new output formats, changed exit codes) that unit tests can't. Trade-off: requires the CLIs installed to run, but that's the point — it validates the actual integration.

**Lifecycle ownership split for iOS.** Rather than duplicating action button rendering in both the detail container and `WaveSessionView`, the iOS path passes `showsSuggestedActions: false` and `managesLifecycle: false` to `WaveSessionView`. The detail container owns session lifecycle (`onAppear`/`onDisappear`/`configureClientContext`) so actions stay live across tab switches (Output ↔ Chat). macOS callers use the defaults and are unaffected.

**Recursive init fix in `RepoState+iOS.swift`.** Changed `self.init()` to `self.init(startBundledDaemon: nil, shellCommandRunner: nil)` to avoid infinite recursion.

## How it fits together

The auth harness (`scripts/test_auth_live_contract.py`) depends on `scripts/lib/lfd_runtime.py` for managed lfd lifecycle. It validates the contract that wave item 03 (Connections Panel) will build against — the same HTTP endpoints, WebSocket events, and payload shapes. The Rust hardening ensures those contracts hold against real-world CLI output variations.

The iOS action rail completes the pre-req for wave/mobile 03 (multi-client). Both macOS and iOS now render suggested actions via the same `SessionState.sendSuggestedAction` path, which means multi-client testing can proceed with feature parity.

## Risks and bottlenecks

- **CLI drift.** The auth harness depends on `gh`, `claude`, and `codex` CLI output formats. If a provider changes their device-auth flow output, the Rust parsers and the harness expectations break together — which is the intended failure mode.
- **iOS action rail untested on hardware.** The bottom safe-area rail is built and compiles, but hasn't been visually verified on iPhone/iPad simulators for thumb-zone spacing and keyboard overlap. Tracked in `wave/mobile/03-multi-client.md`.
- **`reports/` gitignored.** Evidence output goes to `reports/auth-live/` which is gitignored. This is correct for CI artifacts but means evidence must be explicitly preserved if needed for auditing.

## What's not included

- No Concerto Connections Panel UI (that's wave item 03, status: next).
- No multi-client protocol work (wave/mobile 03).
- No changes to the auth HTTP API surface — only client-side validation and parsing hardening.

## Validation

| Check | Result |
|-------|--------|
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy -p loopflow --all-targets -- -D warnings` | Pass |
| `cargo test -p loopflow provider_auth` | 10/10 pass |
| `swift test --package-path swift` | 160/160 pass |
| `xcodebuild build -scheme Concerto` | Clean build |
| `uv run python -m py_compile scripts/test_auth_live_contract.py` | Pass |

## Wave alignment

Advances wave/lfd-repos items 02 (shipped) and 03 (unblocked). Advances wave/mobile item 03 pre-req (iOS action buttons shipped). No new risks introduced against the wave README's identified risks.
