# Gate review: jack-heart.waves-outward

## What was implemented

- Added `lf goal <wave> --tmux` as a deterministic local launch primitive: it creates or reuses `../<repo>.<wave>`, starts the goal loop in tmux with that worktree as cwd, prints the handle, and reprints the handle when the tmux session already exists.
- Moved wave execution state from flat wave fields into `repos: [RepoWork]` across Rust DTOs, Rust store rows, Python models, Swift models, and shared fixtures.
- Reshaped Concerto into the `WavesView` surface: repo rail, disk-authored wave list, and embedded tmux attach path using bundled `lf` instead of lfd run/attach routes.
- Trimmed the old native-chat and multipane macOS view stack, plus stale Swift tests for deleted surfaces.
- Added `ux-research` loop steps and reorganized wave state into current `wave/concerto`, `wave/goals`, `wave/release`, and `wave/website` goals.

## Key choices

- `lf goal --tmux` names sessions from the worktree basename (`lf-<repo>-<wave>`), matching Concerto's derived handle and lfd's `tmux_session_name` sanitizer.
- Worktree creation uses a stable `{user}.{name}` branch. Gate tightened this path so an existing stable branch without a worktree is reused with `git worktree add <path> <branch>` instead of failing.
- Concerto treats disk-authored waves as the baseline and lfd waves as an optional overlay. Running state for disk rows comes from `tmux has-session`.
- The Swift single-repo convenience initializer remains for local tests/placeholders, while wire DTO parsing requires the new `repos` field.
- Stale tests for deleted UI components were removed rather than preserving tests for code intentionally cut from the branch.

## How it fits together

Rust owns the launch contract: resolve the main repo, derive the wave worktree, ensure it exists, launch `lf goal <wave>` inside tmux, and print the tmux handle. Concerto mirrors only the deterministic naming/path rules needed to show running state and attach with Ghostty; lfd remains available for live overlays but is not required for the launch/attach path.

The wave wire model now carries identity at the wave level and execution state per repo. Store reads stitch `wave_repos` into `Wave.repos`; HTTP DTOs expose `RepoWorkDto`; Python and Swift models parse the same nested fixture.

## Risks and bottlenecks

- `lf goal --tmux` depends on `tmux`, git worktrees, and the bundled `lf` path being available in the GUI process environment.
- Existing local lfd databases are not migrated compatibly; this branch follows the hard-cut internal-data posture in the design doc.
- `lf goal --tmux` does not register sessions in lfd yet, so lfd-backed live status still cannot see these client-launched sessions.
- The hard DTO cut means downstream callers still reading `wave.repo` need to move to `wave.repos[0].repo` or repo membership checks.

## What's not included

- No `lfdb` extraction.
- No `lf d` / `lf q` namespaces.
- No deletion of lfd's executor or subscription server.
- No self-registration/session registry for `lf` goal sessions.
- No proactive Concerto worktree pre-allocation.

## Validation

Passed:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `cargo test -p loopflow goal::tests`
- `uv run python -m pytest python/tests/` (`144 passed`)
- `swift test --package-path swift` (`280` Swift Testing tests plus XCTest shim passed)
- `uv run python -m pytest tests/regression/test_session_dto_exposes_tmux_name.py -v`
- `uv run python -m pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` (`13 passed`)
- `tests/e2e/test_smoke.sh`

Attempted but not green locally:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - Built and ran the unit test bundle, but the run failed after the UI runner could not initialize: `Authentication canceled. Canceled by user.`
  - The same run also recorded `KeyboardRouterTests` chord-timeout assertions after an 85s delayed test pass; `swift test --package-path swift` passed that suite normally.
