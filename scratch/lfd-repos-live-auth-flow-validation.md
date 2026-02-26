# 02: Live Auth-Flow Validation

## Problem

Phase 01 proved the auth broker in unit tests, but not against real `gh`, `claude`, and `codex` binaries. The remaining gap is contract drift: if CLI output or credential layouts change, `/v0/auth` can silently break for both `lfq auth` and the upcoming Concerto Connections panel.

This step exists to advance the wave vision: **“Connecting GitHub, Claude, and Codex should be browser-first and identical across clients: click connect, finish auth, continue working.”**

## Approach

Build a **live contract harness + fixture promotion loop** for provider auth.

1. Add `scripts/test_auth_live_contract.py` to run one-command live validation against a local `lfd` instance.
2. For each provider (`github`, `claude`, `codex`), validate end-to-end contract behavior through HTTP + WS:
   - `POST /v0/auth/:provider` returns URL fields (`verification_uri`, `verification_uri_complete`) and optional `user_code` without requiring manual copy/paste.
   - `GET /v0/auth` transitions through `pending` and ends in `active` or `none`/`expired` as expected.
   - Auth lifecycle events arrive in-order: `auth.flow_started` then `auth.connected` or `auth.failed`.
3. Capture raw CLI auth transcript lines and resulting credential tree snapshots during live runs, then convert newly observed output/file patterns into deterministic Rust regression tests in `provider_auth.rs`.
4. Add a Claude-specific disconnect validation case:
   - Seed `.claude/settings.json` + auth-like files.
   - Call `DELETE /v0/auth/claude`.
   - Assert auth artifacts are removed while non-auth settings remain.
5. Emit a provider matrix summary (pass/fail + mismatch reason + captured evidence path) so failures are actionable instead of anecdotal.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Manual QA checklist only | Fast to start, weak repeatability | Regressions reappear because observations are not executable. |
| CI-only live auth tests | Strong gate if feasible | Not practical: interactive/browser auth + real credentials do not fit reliable CI. |
| Fixture-only tests (no live runs) | Deterministic and cheap | Misses upstream drift; fixtures age out without periodic live refresh. |

## Key decisions

- **Choose hybrid validation (live harness + regression fixtures).** Live runs detect drift; fixture tests prevent reintroducing fixed drift.
- **Treat event ordering as part of the public contract.** Connections UI depends on `auth.flow_started` → terminal event semantics.
- **Be strict on API shape, tolerant on raw CLI prose.** We lock down returned fields/events, not exact human-facing CLI wording.
- **Keep this local/release validation, not CI-gated.** Reliability beats false confidence from flaky credential-dependent CI.
- **Wild success target:** onboarding becomes boringly reliable; users click connect and never think about parser details.
- **Wild failure to avoid:** six months of silent drift where auth “mostly works” but fails per-provider; this design counters that by storing evidence and promoting drift into tests.
- **New risk introduced:** stale live fixtures could normalize broken behavior. Mitigation: record CLI version metadata with each transcript and review fixture updates in PR.

## Scope

- In scope:
  - Live validation for `gh`, `claude`, and `codex` through `POST /v0/auth/:provider`.
  - URL/code payload checks (`verification_uri(_complete)`, `user_code`).
  - `GET /v0/auth` status transitions and `auth.connected` / `auth.failed` events.
  - Claude disconnect behavior against real credential-style file layouts.
  - Regression tests for any parser/heuristic changes discovered live.
- Out of scope:
  - Concerto UI work (Step 3).
  - New providers.
  - Provider token refresh behavior.
  - Remote hosted auth orchestration.

## Done when

This wave step is complete when all are true:

1. `uv run python scripts/test_auth_live_contract.py --providers github,claude,codex` passes locally and prints a green provider matrix.
2. The run proves the roadmap step **“Live CLI contract validation + auth-flow hardening (`gh`/`claude`)”** with captured evidence in `reports/auth-live/`.
3. Any newly discovered CLI output or credential-layout drift is encoded in Rust regression tests (for parser/filesystem heuristics), and those tests pass with:
   - `cargo test -p loopflow provider_auth`
