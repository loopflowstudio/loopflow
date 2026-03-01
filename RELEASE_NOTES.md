# v0.9.6

Loopflow 0.9.6 ships token-based mobile auth, usage visibility with cost tracking, and API key support — backed by daemon hardening, comprehensive test coverage, and a restructured documentation journey from first step to autonomous waves.

## New capabilities

- **Connection tokens for mobile** — lfd mints opaque tokens locally instead of validating through studio. SQLite-backed token ledger with 1-hour TTL, session reuse across reconnects, and `lfq token revoke` for revocation. Concerto macOS gets a "Connect with my phone" toggle in Connection Settings
- **Usage and billing visibility** — `lfq usage` shows token spend grouped by wave, flow, step, or model. `--billing` splits subscription vs metered with dollar cost estimates. `--prompt` breaks down token composition by source. Smart group-by inference: `--wave engbot` auto-groups by step
- **API key auth** — per-provider credential type tracking (OAuth vs API key). `lfq auth configure claude` reads keys from environment with clear billing warnings. OAuth remains default; connecting via OAuth automatically switches back from API key
- **Decomposed release workflow** — `lf ops release` splits into five idempotent commands: `release-check`, `release-notes`, `release-bump`, `release-tag`, `release-status`. The `lf release` step orchestrates them with agent judgment for notes and mechanical execution for everything else. Cron waves enable automated daily patches and monthly minors
- **Wave authoring guide** — new standalone documentation covering wave creation (Concerto, lfq, Python API), directory structure, trigger types, and a worked example. Getting-started rewritten as a clear journey: try it, build features, scale with waves, go remote

## Improvements

- **Fast-path rebase** — `lf rebase` runs the mechanical operation first; agent only spins up when conflicts exist. Supports `--onto feature` for explicit targeting
- **Land with PR control** — `lf ops land` accepts `--title` and `--body` flags so agents write PR messaging during the land step. Without flags, enables auto-merge on the existing PR. `ship` and `ship-roadmap` flows now end with `land`
- **Land stages uncommitted changes** — `lf land` picks up uncommitted work before merging, improving end-of-PR cleanup
- **Worktree pruning** — `scratch/` files no longer block cleanup of landed worktrees, squash-merged branches get their own status label, land rotation bases the next worktree on the feature branch instead of main
- **Docs accuracy pass** — fixed stale API signatures, flow format references (Python to YAML), wave API method names, and various command examples across getting-started, config, and waves pages
- **Polished DMG installer** — `create-dmg` builds the Concerto DMG with a custom background and drag-to-Applications layout

## Reliability

- **Transactional migrations** — SQLite migrations now run in `BEGIN EXCLUSIVE/COMMIT` with rollback on failure
- **Daemon test coverage** — inline `#[cfg(test)]` modules for all trigger loops (cron, loop_ticker, recovery, watch), wave CRUD HTTP handlers, and session handler tests. Store basic suite runs against both SQLite and Postgres
- **Resource leak fixes** — output logs pruned on configurable TTL, file handles released after runs, reconcile locks cleaned up on wave deletion

## Fixes

- **Worktree gitlink** — removed accidental worktree gitlink (mode 160000) that broke CI checkout
- **R2 credentials** — strip whitespace from credentials to prevent silent S3 auth failures
