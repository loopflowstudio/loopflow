# LOO-1105: tighten repo skills, split scripts/testing.md, add ci-fix overlay

Prose and skill files only. No Rust, Swift or builtin skill changes, so no
`golden_prompt` snapshots move.

## Outcome

- An `implement` turn stops loading `scripts/testing.md` (486 lines, 28 KB) and
  carries its own focused-test commands, real crate/Swift map and DTO/iPhone
  parity rules (~2 KB).
- `gate` loads a `scripts/testing.md` about 210 lines shorter (~16.3 KB); build
  and cache guidance lives in `scripts/build_cache.md`, read on demand.
- `ci-fix` runs with a self-contained repository overlay naming this
  repository's CI checks, verified local reproductions and `required-integration`
  (~2.6 KB), pointing into `scripts/testing.md` on demand when a local Rust/Mac
  reproduction hits a trap instead of force-loading Gate's planning guide on
  every repair turn.
- `gate` runs Rust lint when Rust changed and Desktop DTO + iOS checks whenever
  the branch reaches iOS/protocol inputs (`scripts/test.py --list` selects
  `ios`, or `rust/loopflow-model/` changed), with a clean fix-and-commit
  recovery path. `kickoff` plans against seven surfaces and `DESIGN.md`. `qa`
  checks for a locked display up front and routes accurately to the locked-screen
  techniques in `.lf/qa/desktop.md`.

## Findings the design rests on

- **The overlay reaches landing's ci-fix.** Landing launches
  `lf --batch --output-schema … skill ci-fix <message>` in the Task checkout
  (`rust/loopflow-execution/src/ops/pr_landing.rs:338`). A probe file
  `.lf/skills/ci-fix.md` with `extends: builtin` rendered through `lf help ci-fix`
  as `ci-fix — skill (.lf/skills/ci-fix.md)`: builtin body first, overlay after.
  No code change needed.
- **`required-integration` is lf's, not GitHub's.** `pr_landing.rs:233`
  synthesizes it when the PR needs a resync (sibling to `merge-queue (…)` and
  `merge-request (…)` in builtin `ci-fix.md`). There is no log to read; the
  repair is `lf sync`, then `lf arm`. Keeping it in `.lf/skills/ci-fix.md`
  fixes this repo now without touching builtins/goldens; moving it into builtin
  `ci-fix.md` is a separate follow-up Task.
- **Actual Swift & Rust wire layout.**
  - `swift/` has three product targets: `swift/Loopflow/` (shared library:
    DTOs, server client, themes, shared views), `swift/LoopflowMac/` (macOS
    Desktop shell + GhosttyKit), `swift/LoopflowiOS/` (iPhone app shell). There
    is no `swift/LoopflowDesktop/`.
  - `rust/loopflow-model/` holds only 6 small domain files (`durable.rs`,
    `id.rs`, `comment.rs`, `person.rs`, `program_status.rs`, `chat/`). Most
    server/CLI wire DTOs live in `rust/loopflow/src/server/`,
    `rust/loopflow-execution/src/{machine,transport,terminal_holder}/wire.rs`,
    `read_stream.rs`, and `rust/loopflow/src/lf/commands/{read,write}.rs`.
  - `scripts/macos_inputs.py` (`_PROTOCOL_INPUTS` / `IOS_INPUTS`) is the
    repository's single classifier for what reaches Swift/iOS, and
    `scripts/test.py --list` imports `IOS_INPUTS` directly (`scripts/test.py:41`).
    `rust/loopflow-model/` is not yet in `_PROTOCOL_INPUTS`, so `gate.md`
    checks "`scripts/test.py --list` selects `ios`, or `rust/loopflow-model/`
    changed".
- **Exact CI jobs and verified local reproductions** (read directly from
  `.github/workflows/ci.yml` and `.github/workflows/macos.yml`):
  - `required-integration`: synthetic `lf` check (`pr_landing.rs:233`), no
    GitHub log. Run `lf sync`, verify clean/tests, then `lf arm`.
  - `scratch-clear`: merge-queue/main rejects any file under `scratch/` other
    than `.gitkeep` (a `lf sync` conflict resolution can restore notes). Run
    `lf scratch clear`, commit if needed, then `lf arm`.
  - `architecture-check`: fetch `origin/main`
    (`git fetch --no-tags origin main:refs/remotes/origin/main`), then
    `uv run --managed-python python scripts/check_architecture.py` (needs
    managed Python's SQLite ≥ 3.47 for `RAISE` expressions).
  - `rust-lint`: `cargo fmt --all -- --check` and
    `cargo clippy --all-targets -- -D warnings`. CI runs rustup `stable`; if
    CI fails after a local pass, prepend `$HOME/.cargo/bin` to `PATH` so
    rustup's Clippy runs instead of Homebrew's (`rust/loopflow/tests/README.md`).
    Linux-only code (`cfg(target_os = "linux")`) is linted only in CI; read the
    log for those.
  - `migration-check`:
    `git fetch --no-tags origin main:refs/remotes/origin/main`, then
    `LOOPFLOW_REQUIRE_ORIGIN_MAIN=1 UV_PYTHON_PREFERENCE=only-managed uv run python scripts/check_migrations.py`
    and `UV_PYTHON_PREFERENCE=only-managed uv run python scripts/schema_page.py --check`.
  - `macos / swift-test`:
    - `scripts/test_desktop.sh --filter '<failed suite>'` (already runs
      `cargo build -p loopflow --bin lf` unless `--skip-build` is passed)
    - `uv run pytest python/tests/test_desktop_performance.py::test_checkout_observation_preserves_read_boundary_and_detects_changes`
    - `uv run python scripts/check_desktop_launch.py`
    - `uv run python scripts/check_swift_multiplatform_boundaries.py`
  - `macos / loopflow-ui-test`: two halves (`macos.yml:229-252`):
    - Desktop Xcode compile check:
      `(cd swift && xcodegen generate) && uv run python scripts/build_xcode_ci.py`
    - iOS Simulator tests: `uv run python scripts/test_ios.py` (builds `lf`,
      resolves SwiftPM, runs `xcodegen --spec swift/ios.yml`, creates a
      disposable Simulator; pass `-only-testing:LoopflowiOSTests/<Suite>` to
      focus).
  - `macos / macos-process-test`:
    `uv run python scripts/test_network.py cargo test --offline -p loopflow --lib <app_process_tests | journal::os_process:: | server::mobile::tailscale::macos::>`
  - `task-installation`: `uv run python scripts/test_task_installation.py`
    (CI passes `--container` and also `--repository-upgrade`; read
    `scripts/README.md` for local usage).
  - `e2e-smoke`: runs TWO steps (`ci.yml:430-432`): first
    `planning_reconnect_tests` (Linux only — read CI log on macOS), then
    `uv run python scripts/test_network.py tests/e2e/test_smoke.sh` after
    `cargo build -p loopflow --bin lf`.
- **`documented_commands` does not read `.lf/skills/` or `scripts/`.** It scans
  `README.md`, `AGENTS.md`, `docs/`, builtins and top-level `skills/`
  (`rust/loopflow/tests/documented_commands.rs:78`). Only the `AGENTS.md` edit
  is in its reach. Every `lf` command quoted in an overlay is checked by hand
  with `lf help <path>`.
- **Display lock reads headless in one line.**
  `ioreg -n Root -d1 | grep -oE '"(IOConsoleLocked|CGSSessionScreenIsLocked)"\s*=\s*\w+'`
  printed `"IOConsoleLocked" = No` on this unlocked Mac with no XML parsing.
  Checking both keys covers both `IOConsoleLocked = Yes` and
  `CGSSessionScreenIsLocked = Yes` (treat either `Yes` as locked, `No`/absent
  as unlocked; note that only the unlocked reading was observed headlessly).
- **`.lf/qa/desktop.md` locked-screen reality.**
  - Headless automated checks (`scripts/test_desktop.sh`), `lf desktop state --json`,
    `-ui-test-mode live` snapshots (lines 151–193), and production-mode
    `QAHook` snapshots/clicks/views (lines 195–520) all work while locked.
  - Production mode + `QAHook` **does** open a Ghostty surface and drives
    keyboard input (`key post` / `key send`, lines 353–385); only
    `-ui-test-mode live` lacks Ghostty panes (line 188).
  - Genuinely unreachable while locked: hover, system drags, menus / `⌘`
    shortcuts (no key window), popovers/sheets opened from "+", file panels,
    Settings pixel rendering (white RenderBox; use `views`), and Ghostty
    focus-in / visual terminal face verification (lines 188, 347–351, 368–375,
    456–458).
- **`.lf/qa/mobile.md` breaks the `head -n 1 .lf/qa/*.md` routing header.** Its
  first line is `# Phone access against the Mac's real Tailscale` instead of
  `Read when …`.
- **Anchors that move** with the Builds section:
  `rust/loopflow-execution/README.md:30` (`#builds`), `scripts/README.md:42`,
  `scripts/benchmarks/crate-rebuild/README.md:25,76`
  (`#shared-compilation-cache`). Relative links inside the moved text
  (`benchmarks/…`) keep working because `scripts/build_cache.md` sits in
  `scripts/` too.
- `scripts/testing.md` section sizes today: Header (5 L, 189 B), Plan first
  (137 L, 7.9 KB), Implement (21 L, 1.2 KB), Gate (83 L, 5.1 KB), macOS traps
  (34 L, 2.2 KB), Builds + Shared compilation cache (188 L, 10.5 KB), Leave to
  CI (6 L, 282 B), Boundary checks (12 L, 533 B).
- `DESIGN.md`, `design/prototypes/`, `scripts/test_ios.py`,
  `swift/LoopflowTests/DTOFixtureTests.swift` exist.

## Design

### scripts/testing.md → three consumers

| Consumer | Gets | Loaded how |
|---|---|---|
| `implement` | focused-test commands, `LF_*`/stdin rule, crate/Swift map, DTO & iPhone parity inline in `.lf/skills/implement.md`; `docs:` removed | ~2 KB inline |
| `gate` | slimmed `scripts/testing.md` (~275 lines, ~16.3 KB) | `docs: scripts/testing.md` |
| `ci-fix` | verified CI check table + `LF_*`/stdin rule inline in `.lf/skills/ci-fix.md`; pointer naming Mac error strings (`syspolicyd`, `TMPDIR`, `has no resident login`, `No agent selected`) to `scripts/testing.md` | ~2.6 KB inline; `scripts/testing.md` on demand |
| anyone building or debugging the cache | `scripts/build_cache.md` (188 lines) | by link from `scripts/testing.md`, `AGENTS.md`, `scripts/README.md`, `rust/loopflow-execution/README.md`, `scripts/benchmarks/crate-rebuild/README.md` |

- `scripts/build_cache.md` takes lines 281–467 whole (`## Builds` and
  `### Shared compilation cache`), promoted one heading level (`# Builds`,
  `## Shared compilation cache`). Nothing is rewritten; this is a move.
- `scripts/testing.md` keeps Plan first, Gate, macOS traps, Leave to CI,
  Boundary checks; gains a one-line link to `build_cache.md`; its opening line
  names `gate` (always loaded) and `ci-fix` (read on demand for Mac traps and
  nextest filters).
- The "Implement: one focused test" section leaves `scripts/testing.md`. Its
  command block and "never the whole workspace" line move into `implement.md`;
  its decision-rows paragraph moves under Plan first.
- `AGENTS.md:200` points at both `scripts/testing.md` and
  `scripts/build_cache.md`.

### .lf/skills/implement.md

Removes `docs: scripts/testing.md`. Adds, above the existing UI rules:

- Focused-test commands (library module, harness decoder, integration module,
  standalone binary, Python file), the never-`cargo test --all` /
  `cargo nextest run --all` line, and the `LF_*` unset + `< /dev/null` rule
  with the inline zsh form (`env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /')`).
- Accurate crate and surface map, one line each:
  - `rust/loopflow-model` — shared domain/ID/status types (wire-facing; changes reach Swift)
  - `rust/loopflow-storage` — SQLite store, migrations, `SCHEMA.md`
  - `rust/loopflow-harness` — provider decoders, transcripts (builds no storage or execution)
  - `rust/loopflow-execution` — launch, skills, shipping, wire/transport types
  - `rust/loopflow` — CLI, server (`src/server/`), integration tests
  - `swift/Loopflow` — shared Swift library (DTOs, server client, themes, shared views used by Mac and iOS)
  - `swift/LoopflowMac` — macOS Desktop shell and GhosttyKit integration
  - `swift/LoopflowiOS` — iPhone app target (`scripts/test_ios.py`)
  - `website/` — site and generated CLI reference
- **DTO parity**: a wire field change (in `rust/loopflow-model/`,
  `rust/loopflow/src/server/`, `wire.rs`, `read_stream.rs`, or CLI `--json`
  reads/writes) updates `tests/fixtures/dto/`, Rust + Swift required-nullable
  decoders (`swift/Loopflow/`), then runs
  `scripts/test_desktop.sh --filter DTOFixtureTests` and
  `uv run python scripts/test_ios.py`.
- **iPhone parity**: a change to shared Swift views, themes, models or Task
  status (`swift/Loopflow/`) is checked in the iOS target too
  (`uv run python scripts/check_swift_multiplatform_boundaries.py` and
  `uv run python scripts/test_ios.py`).

### .lf/skills/ci-fix.md (new)

`extends: builtin` (no `docs:` attachment). One "In this repository" section:

- The verified check-to-command table from Findings (`required-integration`,
  `scratch-clear`, `architecture-check`, `rust-lint`, `migration-check`,
  `macos / swift-test`, `macos / loopflow-ui-test`,
  `macos / macos-process-test`, `task-installation`, `e2e-smoke`).
- The `LF_*` unset + `< /dev/null` rule (inline zsh form).
- One on-demand pointer: when reproducing a failing Rust or macOS test locally
  hits `syspolicyd`, `TMPDIR`, `fseventsd`, `has no resident login`,
  `No agent selected`, or needs nextest `--rust-filter` / path-to-test traps,
  read `scripts/testing.md`.

### .lf/skills/gate.md

Updates the preparation block and recovery flow:

```bash
lf commit -m "gate: checkpoint"     # working notes are in history here
lf sync                             # last thing before checks & publish (after review fixes too)
# When Rust changed (fix & commit if either fails):
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings
# When scripts/test.py --list selects ios, or rust/loopflow-model/ changed:
scripts/test_desktop.sh --filter DTOFixtureTests
uv run python scripts/test_ios.py
# When swift/ changed, also:
uv run python scripts/check_swift_multiplatform_boundaries.py
lf scratch clear                    # branch-added files only; base files stay
lf pr publish                       # ready PR, no merge request
lf pr checks                        # rust-test, tests-result: not skipping
```

Makes the recovery rule explicit: if lint or a check requires a fix, commit the
fix before `lf pr publish`; after pass-2 review fixes, run `lf sync`, re-run
applicable local checks, confirm `scratch/` still contains only `.gitkeep` (a
sync can restore notes), commit, then `lf pr publish`.

`AGENTS.md:448` ("When to run what" → gate) updates to match: run Rust lint
(when Rust changed) and DTO/iOS parity (when wire/shared Swift changed) before
publishing; CI runs the suites while gate reviews.

### .lf/skills/kickoff.md

Adds:
- **Blast-radius checklist**: name what the change does to each of CLI, Server
  RPC and schema, Storage and migrations, DTOs and fixtures, Desktop
  (`swift/LoopflowMac` + shared `swift/Loopflow`), iPhone (`swift/LoopflowiOS`
  + shared `swift/Loopflow`), and Docs — or say it doesn't reach it.
- **UI design reference**: UI Tasks (Desktop, iPhone, website) read `DESIGN.md`
  and `design/prototypes/` first.

### .lf/skills/qa.md and .lf/qa/

- Move "Starting a Task on a private Machine" (`qa.md:23-52`) into
  `.lf/qa/served-launches.md` and widen its `Read when` line to cover starting
  a Task on a private Machine. Move "Shared build cache" (`qa.md:54-64`) into
  `.lf/qa/build-cache.md`, replacing its "see the qa skill's shared-cache
  notes" pointer. Every sentence survives; `qa.md` keeps one routing line for
  CLI and server changes pointing to `.lf/qa/served-launches.md` and
  `.lf/qa/server.md`.
- Add `Read when the change touches phone access over Tailscale.` as line 1 of
  `.lf/qa/mobile.md` so `head -n 1 .lf/qa/*.md` indexes it cleanly.
- Display-lock preflight opens `qa.md`'s Desktop section and `.lf/qa/desktop.md`:
  ```bash
  ioreg -n Root -d1 | grep -oE '"(IOConsoleLocked|CGSSessionScreenIsLocked)"\s*=\s*\w+'
  ```
  - **Locked** (either key reads `Yes`): skip interactive GUI/WindowServer
    clicks and go straight to headless `scripts/test_desktop.sh`,
    `lf desktop state --json`, `-ui-test-mode live` snapshots, or production
    `QAHook` (which covers windows, clicks, views, and Ghostty/keyboard via
    `key post`/`key send`). Report hover, system drags, menus/`⌘` chords,
    "+" popovers/sheets, file panels, Settings pixels, and visual terminal-face
    checks as open instead of retrying them.
  - **Unlocked** (`No` or absent): proceed with isolated Dev bundle checks.

## Choices made for review

- **Omit `docs: scripts/testing.md` from `.lf/skills/ci-fix.md` and point to it
  on demand.** Keeps `ci-fix` at ~2.6 KB instead of force-loading ~16.3 KB of
  Gate planning prose onto all ~272 repair turns/14d; naming the Mac error
  strings beside the pointer catches the counterexample. Passed over: attaching
  `docs: scripts/testing.md` to `ci-fix.md`.
- **Trigger gate's pre-publish DTO/iOS checks from `scripts/test.py --list`
  selecting `ios` (plus `rust/loopflow-model/`).** Reuses `macos_inputs.py`'s
  single protocol/Swift classifier so server/wire edits that forget
  `tests/fixtures/dto/` don't slip through. Passed over: gating only on
  `rust/loopflow-model/` or `tests/fixtures/dto/`.
- **Lint runs only when Rust changed, and after `lf sync`.** Prose-only
  branches pay no clippy build, and post-sync lint catches breakage from new
  callers on `main`. Passed over: unconditional lint or pre-sync lint.
- **One copy of the focused-test commands in skill docs.** Moves to
  `implement.md` and leaves `scripts/testing.md`; `AGENTS.md`'s short block
  stays. Passed over: duplicating in both.
- **Keep `required-integration` in `.lf/skills/ci-fix.md` without editing
  builtin `ci-fix.md`.** Keeps this Task prose-only with no `golden_prompt`
  churn; moving it to builtin `ci-fix.md` is a separate Task.
- **The whole Builds section moves**, not only the cache subsection (lines
  281–467).

## Exclusions

- `AGENTS.md`'s 512 lines stay, apart from the two touched sentences (line 200
  and line 448).
- No builtin skill edits; no change to how `docs:` loads; no `scripts/*.py`
  edits (adding `rust/loopflow-model/` to `scripts/macos_inputs.py` and moving
  `required-integration` into builtin `ci-fix.md` are separate follow-ups).
- `.lf/skills/hotfix.md` and the other overlays are untouched.
- No new lock-screen technique; no change to `QAHook.swift`.

## What goes

- `docs: scripts/testing.md` in `.lf/skills/implement.md`.
- `scripts/testing.md` lines 281–467 (moved to `scripts/build_cache.md`) and
  its "Implement: one focused test" section (moved to `.lf/skills/implement.md`
  / Plan first).
- `.lf/skills/qa.md` lines 23–64 (moved into `.lf/qa/served-launches.md` and
  `.lf/qa/build-cache.md`).
- Must survive: every sentence of the moved sections; every existing link to
  `scripts/testing.md`; `docs/config.md` and `docs/orchestration.md` examples
  (`docs: [scripts/testing.md]` stays true for gate).

## One pass

Order:
1. Split `scripts/testing.md` into `scripts/build_cache.md`, update the four
   anchors + `AGENTS.md`.
2. Update `.lf/skills/implement.md`, `.lf/skills/gate.md`, `.lf/skills/kickoff.md`,
   and add `.lf/skills/ci-fix.md`.
3. De-duplicate `.lf/skills/qa.md` into `.lf/qa/served-launches.md` and
   `.lf/qa/build-cache.md`, add the display-lock preflight to `qa.md` and
   `.lf/qa/desktop.md`, and add the `Read when` line to `.lf/qa/mobile.md`.
4. Verify every `lf` command and anchor below.

## Acceptance at gate

```bash
U=$(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /')
for s in implement gate ci-fix qa kickoff; do env ${=U} lf help $s < /dev/null | head -1; done
# five lines; ci-fix reads "skill (.lf/skills/ci-fix.md)"
! grep -q '^docs:' .lf/skills/implement.md
! grep -q '^docs:' .lf/skills/ci-fix.md
grep -q '^docs: scripts/testing.md' .lf/skills/gate.md
grep -rn 'testing.md#' --include='*.md' . | grep -v '^./\(target\|release\|\.lf/prompts\|scratch\)'
# every anchor printed exists as a heading in scripts/testing.md
grep -rn 'build_cache.md' AGENTS.md scripts/testing.md scripts/README.md rust/loopflow-execution/README.md scripts/benchmarks/crate-rebuild/README.md
head -n 1 .lf/qa/*.md | grep -v '^==>\|^$\|^Read when '
# empty output: every .lf/qa/*.md file starts with "Read when ..."
env ${=U} cargo test -p loopflow --test integration documented_commands:: < /dev/null
wc -l scripts/testing.md scripts/build_cache.md .lf/skills/qa.md .lf/skills/ci-fix.md
```

## Demo

`lf help ci-fix` prints the repository's verified CI check table under the
builtin procedure without attaching 16 KB of Gate prose, and `lf help implement`
prints the focused-test commands and real crate/Swift map with no
`scripts/testing.md` attached. Before and after: `wc -c scripts/testing.md`
(28 KB today) against what `implement` loads (~2 KB inline, 0 attached),
`ci-fix` loads (~2.6 KB inline, 0 attached), and `gate` loads (~16.3 KB).

## Status

Implemented in one pass (`301c353c14`, `7b89ed9596`, `d3d4485922`); nothing left to build. Ready for gate.

Measured sizes:
- `scripts/testing.md`: 289 lines, 17,028 B (down from 486 lines, ~28 KB)
- `scripts/build_cache.md`: 187 lines, 10,515 B (verbatim move of Builds + Shared compilation cache)
- `.lf/skills/implement.md`: 69 lines, 3,414 B, 0 `docs:` attached (was 29 lines + 28 KB attached)
- `.lf/skills/ci-fix.md`: 25 lines, 2,789 B, 0 `docs:` attached
- `.lf/skills/gate.md`: 51 lines, 2,362 B, `docs: scripts/testing.md` attached
- `.lf/skills/kickoff.md`: 26 lines, 1,179 B
- `.lf/skills/qa.md`: 42 lines, 1,918 B (down from 64 lines, 3,624 B)

Checks run:
- `lf help {implement,gate,ci-fix,qa,kickoff}`: all five resolve to `.lf/skills/<name>.md`
- `docs:` frontmatter check: absent in `implement.md` and `ci-fix.md`, `scripts/testing.md` in `gate.md`
- `testing.md#` anchor grep: zero remaining
- `build_cache.md` link grep: all five targets updated (`AGENTS.md`, `scripts/testing.md`, `scripts/README.md`, `rust/loopflow-execution/README.md`, `scripts/benchmarks/crate-rebuild/README.md`)
- `head -n 1 .lf/qa/*.md`: every file starts with `Read when `
- `uv run --managed-python python scripts/check_architecture.py`: PASS (zero unexplained items)
- `cargo test -p loopflow --test integration documented_commands::`: 4 passed, 0 failed
