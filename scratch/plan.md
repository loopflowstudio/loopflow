# LOO-1105: tighten repo skills, split scripts/testing.md, add ci-fix overlay

Prose and skill files only. No Rust, Swift, builtin skill or `scripts/*.py`
changes, so no `golden_prompt` snapshots move.

## Built (`301c353c14`, `7b89ed9596`, `d3d4485922`)

- `scripts/testing.md` → `gate` only (attached) and `ci-fix` (on demand, named
  with the Mac error strings). Builds + Shared compilation cache moved verbatim
  to `scripts/build_cache.md`; the four anchors and `AGENTS.md` point there.
- `.lf/skills/implement.md`: no `docs:`; inline focused-test commands, `LF_*`
  rule, crate/Swift map (`swift/Loopflow`, `swift/LoopflowMac`,
  `swift/LoopflowiOS`; there is no `swift/LoopflowDesktop/`), DTO and iPhone
  parity.
- `.lf/skills/ci-fix.md` (new, `extends: builtin`, no `docs:`): verified table
  of this repository's CI checks read from `.github/workflows/{ci,macos}.yml`,
  including lf's synthetic `required-integration` (`pr_landing.rs:233`).
- `.lf/skills/gate.md`: post-`lf sync` fmt/clippy when Rust changed;
  DTOFixtureTests + `test_ios.py` when `scripts/test.py --list` selects `ios`
  or `rust/loopflow-model/` changed; fix-and-commit recovery.
- `.lf/skills/kickoff.md`: seven-surface checklist, `DESIGN.md` +
  `design/prototypes/` for UI Tasks.
- `.lf/skills/qa.md`: private-Machine and build-cache notes moved to
  `.lf/qa/served-launches.md` and `.lf/qa/build-cache.md`; display-lock
  preflight in `qa.md` and `.lf/qa/desktop.md`; `.lf/qa/mobile.md` starts with
  `Read when`.

## Findings that explain the approach

- A probe `.lf/skills/ci-fix.md` with `extends: builtin` rendered builtin body
  first, overlay after; landing launches `lf --batch … skill ci-fix` in the Task
  checkout (`rust/loopflow-execution/src/ops/pr_landing.rs:338`), so the overlay
  reaches it with no code change.
- `documented_commands` scans `README.md`, `AGENTS.md`, `docs/`, builtins and
  `skills/` only (`rust/loopflow/tests/documented_commands.rs:78`); `lf`
  commands quoted in `.lf/skills/` and `scripts/` were checked by hand.
- `ioreg -n Root -d1 | grep -oE '"(IOConsoleLocked|CGSSessionScreenIsLocked)"\s*=\s*\w+'`
  printed `"IOConsoleLocked" = No` on an unlocked Mac. The locked reading was
  never observed.

## Checks (run 2026-11-25, this checkout)

- `lf help {implement,gate,ci-fix,qa,kickoff}`: each resolves to `.lf/skills/<name>.md`.
- `docs:` count: implement 0, ci-fix 0, gate 1.
- No `testing.md#` links remain; `build_cache.md#builds` and
  `#shared-compilation-cache` targets exist.
- `head -n 1 .lf/qa/*.md`: every file starts with `Read when `.
- `cargo test -p loopflow --test integration documented_commands::`: 4 passed.
- `uv run --managed-python python scripts/check_architecture.py`: pass.
- Sizes: `scripts/testing.md` 289 L / 17,028 B (was 486 L / ~28 KB);
  `scripts/build_cache.md` 187 L / 10,515 B; `implement.md` 3,414 B, 0 attached;
  `ci-fix.md` 2,789 B, 0 attached; `qa.md` 1,918 B (was 3,624 B).

## Remains

- **Gate**: publish; CI runs the suites. No Rust changed, so no local lint.
  Deferred: the locked-display branch of the qa preflight (needs a locked Mac).

## Follow-ups (separate Tasks, not this PR)

- Add `rust/loopflow-model/` to `_PROTOCOL_INPUTS` in `scripts/macos_inputs.py`,
  then drop the extra clause from `gate.md`.
- Move `required-integration` into builtin `ci-fix.md` (golden_prompt change).
