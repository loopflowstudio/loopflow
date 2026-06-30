## Try it!

```bash
swift test --package-path swift --filter Portfolio
swift test --package-path swift
cargo fmt --check
uv run pytest python/tests/test_pm_reset.py -q
cargo +nightly test -p loopflow build_codex_command_without_context_file_omits_model_instructions
RUSTUP_TOOLCHAIN=nightly uv run pytest tests/regression/test_orphaned_runs_reset_wave_status.py tests/regression/test_terminal_session_dto_exposes_tmux_name.py -q
```

The portfolio tests cover legacy decode into Active, fixed tier grouping, midpoint/edge reorder math, and persistence after reload. The full Swift package run passed 338 tests.

Stable Rust is currently blocked in this checkout before branch code runs: `libsqlite3-sys 0.38.1` fails to compile under stable `rustc 1.93.0` because `cfg_select` is still rejected as unstable. The focused Rust test and Rust-backed regression tests pass with the installed nightly toolchain.

## Intent

Make Concerto's portfolio reflect project priority instead of recency. Repos now live in fixed Core, Active, Future, and Deprecated sections with manual order inside each tier. The branch also fixes Codex skill launches by omitting `model_instructions_file` when the generated system prompt is empty.

## Assumptions

- Portfolio data is Swift/UserDefaults UI state, not a mirrored wire DTO.
- Existing persisted repos should migrate into Active without losing visible order.
- Tiers are fixed for this pass; users can reposition repos but cannot edit tier names or create tiers.
- No rendering environment was available for this gate, so drag/drop was validated through model tests and code review rather than visual QA.

## Key decisions

- Store rank as `(tierId, priority)` on each repo and expose one canonical `orderedRepos` sort.
- Use midpoint/edge priority assignment for drag/drop so reorders only update the moved repo.
- Keep all four tiers visible, including empty ones, so they are valid drop targets.
- Keep context-menu tier moves as the keyboard/non-drag fallback.
- Keep tier order derived from `PortfolioTier.all` to avoid duplicated rank data in hardcoded tier constants.

## Not included

- User-editable tiers.
- Repo-name based pre-seeding into Core/Future/Deprecated.
- A visual insertion indicator during drag.
- Concerto UI drag/drop automation.
