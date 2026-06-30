# Portfolio tiers gate review

## What was implemented

Concerto's portfolio dashboard now groups repos into four fixed tiers: Core, Active, Future, and Deprecated. `PortfolioRepo` persists `tierId` and `priority`, `PortfolioService.orderedRepos` owns the canonical rank sort, and `reposByTier()` feeds the window sections so every tier renders even when empty.

Repo cards are draggable, tier sections are drop targets, and card context menus expose a non-drag "Move to tier" path. Existing stored portfolio entries decode into Active with priority derived from `lastOpened`, so old data keeps its visible order on first launch.

This branch also includes a small `lf run`/Codex skill-launch fix: when a prompt build has an empty system prompt, Loopflow no longer writes or passes an empty `model_instructions_file`.

## Key choices

- Tier membership and within-tier order live in persisted portfolio state, not view-local arrays.
- `priority` is a `Double`; reorders assign midpoint/edge values instead of renumbering the tier.
- `lastOpened` is now only a final tiebreak in `orderedRepos`.
- Tiers remain hardcoded. A prior simplification keeps `PortfolioTier.order` derived from the tier table order rather than duplicating order in every tier value.
- `reorder` now ignores neighbor hints that point to the moved repo or to a repo outside the destination tier.
- No real-portfolio name preseed was added; repos start in Active and users place them once.

## How it fits together

`PortfolioWindow` asks `PortfolioService` for tier groups and renders one section per `PortfolioTier.all` entry. Drag/drop and context-menu moves all call `PortfolioService.reorder`, which updates exactly one repo's `tierId` and `priority`, saves to UserDefaults, and lets `orderedRepos` produce the next visible rank.

Legacy migration is local to `PortfolioRepo.init(from:)`: missing `tierId` becomes Active and missing `priority` becomes `-lastOpened.timeIntervalSinceReferenceDate`, preserving the old newest-first order as a rank value.

## Risks and bottlenecks

- Card drop placement uses a simple top-half/bottom-half split. It is enough for MVP reorder slots, but it is not a precise insertion indicator.
- Repeated midpoint insertion between the same two neighbors can eventually lose precision. The current portfolio scale makes this unlikely; a future normalize-on-load pass would be cheap if needed.
- Manual visual drag/drop QA was not available in this headless run. The CI-style Xcode command passed with `ConcertoUITests` skipped, which validates the app/test bundle without requiring the UI runner target.

## What's not included

- User-editable tiers.
- Real portfolio pre-seeding by repo name.
- A visible insertion-line affordance during drag.
- Concerto UI drag/drop automation.

## Validation

- `swift test --package-path swift --filter Portfolio` passed: 8 tests.
- `swift test --package-path swift` passed: 338 tests.
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -skip-testing:ConcertoUITests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` passed.
- `uv run python scripts/check_swift_multiplatform_boundaries.py` passed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test --all` passed.
- `uv run pytest python/tests/` passed: 148 tests.
- `uv run pytest python/tests/test_pm_reset.py -q` passed: 8 tests.
- `tests/e2e/test_smoke.sh` passed.
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` passed: 16 tests.
- `docker version && cargo test -p loopflow docker_ -- --nocapture` passed. Local Docker was available through OrbStack; two live socket-dependent Docker cases self-skipped because `/var/run/docker.sock` was unavailable.
- `cargo test -p loopflow build_codex_command_without_context_file_omits_model_instructions` passed.
- `cargo +nightly test -p loopflow build_codex_command_without_context_file_omits_model_instructions` passed.
- `RUSTUP_TOOLCHAIN=nightly uv run pytest tests/regression/test_orphaned_runs_reset_wave_status.py tests/regression/test_terminal_session_dto_exposes_tmux_name.py -q` passed: 3 tests.
