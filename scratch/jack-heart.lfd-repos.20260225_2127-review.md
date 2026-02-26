# Review: Live Auth Contract Harness + Auth Broker Hardening

Branch: `jack-heart.lfd-repos.20260225_2127`
Wave item: `02-live-auth-flow-validation`

## What was implemented

A live auth contract validation script (`scripts/test_auth_live_contract.py`) that spins up a hermetic lfd instance and exercises the full `/v0/auth/{provider}` lifecycle for GitHub, Claude, and Codex. Alongside this, several auth broker parsing and status-checking paths in `rust/loopflow/src/lfd/provider_auth.rs` were hardened based on real CLI output observed during live testing.

## Key changes

### Live contract harness (`scripts/test_auth_live_contract.py`, 702 lines)

- Starts lfd in an isolated `$HOME` via `LfdRuntime`, then for each provider:
  1. Captures raw CLI transcript (output format, version, timing).
  2. Calls `POST /v0/auth/{provider}` and validates the start payload schema.
  3. Polls `GET /v0/auth/{provider}` for `pending` status.
  4. Listens on websocket for `auth.flow_started` → `auth.connected`/`auth.failed` event ordering.
  5. Calls `DELETE /v0/auth/{provider}` to clean up.
- Claude disconnect sub-test validates that `DELETE /v0/auth/claude` removes auth artifacts (`auth.json`, `oauth-tokens.json`, `session-cache/`) while preserving non-auth settings (`settings.json`, `projects.json`).
- All evidence captured to `reports/auth-live/<timestamp>/` as JSON, JSONL, and credential tree snapshots.

### Auth broker hardening (`provider_auth.rs`, +119 lines net)

| Change | Why |
|--------|-----|
| `USER_CODE_RE`: `{4}` → `{4,}` per segment | Codex emits variable-length segments like `1XH6-DG19Y` |
| ANSI escape stripping before line parsing | `codex login` wraps URLs and codes in ANSI color sequences |
| GitHub status fallback to `~/.config/gh/hosts.yml` | `gh auth status` may not be installed or may fail; filesystem gives a second chance |
| GitHub disconnect tolerates "not logged in" | Idempotent disconnect when already disconnected |
| Claude command: `["claude", "login"]` → `["claude", "auth", "login"]` | Matches current Claude CLI |
| Claude status: `directory_has_entries` → `has_auth_like_entries` | Avoids false positives from non-auth files in `~/.claude/` |
| Claude disconnect: selective removal of auth-like entries | Preserves `settings.json`, `projects.json`, etc. |

### Wave docs

- `02-live-auth-flow-validation.md` updated to **shipped** with summary.
- `03-connections-panel.md` updated to reflect that no server-side auth API work remains — just Swift UI.
- `README.md` roadmap table updated.

## How it fits together

The auth broker (`provider_auth.rs`) is the server-side engine that launches CLI auth flows and translates their output into structured events. The live contract harness validates that the HTTP + websocket API surface behaves correctly end-to-end with real provider CLIs. This is a pre-requisite for building the Concerto Connections Panel (wave item 03) — the Swift UI can now be built with confidence that the API contract is tested.

## Risks and bottlenecks

- **Provider CLI drift**: The harness depends on `gh`, `claude`, and `codex` CLI output formats. If providers change their output, both the regex parsing in Rust and the contract assertions in the harness will need updating. The evidence capture makes debugging straightforward.
- **Not in CI**: The live harness requires provider CLIs installed on the runner. It's a manual validation script, not a CI job. CI coverage comes from the Rust unit tests on the parsing/status logic.
- **Filesystem heuristics**: Claude and Codex status checks use presence of `~/.claude/` and `~/.codex/` entries. These may not match real credential layouts on all platforms.

## What's not included

- No changes to the HTTP API handlers or websocket event types — those were already working.
- No Swift/Concerto UI changes — that's wave item 03.
- The harness doesn't complete real auth flows (that requires browser interaction) — it validates the lifecycle up to and including the terminal event after the CLI exits.
