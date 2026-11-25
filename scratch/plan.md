# LOO-1105: tighten repo skills, split scripts/testing.md, add ci-fix overlay

Prose and skill files only. No Rust, Swift or builtin skill changes, so no
`golden_prompt` snapshots move.

## Outcome

- An `implement` turn stops loading `scripts/testing.md` (486 lines, 28 KB) and
  carries its own focused-test commands, crate map and DTO/iPhone rules.
- `gate` and `ci-fix` load a `scripts/testing.md` about 190 lines shorter; build
  and cache guidance lives in `scripts/build_cache.md`, read on demand.
- `ci-fix` runs with a repository overlay naming this repository's checks and
  how to reproduce each locally.
- `gate` lints Rust before publishing and runs the DTO consumers when the wire
  shape moved. `kickoff` plans against seven surfaces. `qa` checks for a locked
  display before trying a window.

## Findings the design rests on

- **The overlay reaches landing's ci-fix.** Landing launches
  `lf --batch --output-schema … skill ci-fix <message>` in the Task checkout
  (`rust/loopflow-execution/src/ops/pr_landing.rs:338`). A probe file
  `.lf/skills/ci-fix.md` with `extends: builtin` rendered through `lf help ci-fix`
  as `ci-fix — skill (.lf/skills/ci-fix.md)`: builtin body first, overlay after.
  No code change needed.
- **`required-integration` is lf's, not GitHub's.** `pr_landing.rs:233`
  synthesizes it when the PR needs a resync. There is no log to read; the
  repair is `lf sync`, then `lf arm`.
- **Check names match the workflows.** `ci.yml`: `architecture-check`,
  `rust-lint`, `task-installation`, `migration-check`, `e2e-smoke`. `macos.yml`:
  `swift-test`, `loopflow-ui-test` (shown as `macos / …`).
- **What each job runs** (the overlay's local reproductions):
  - `architecture-check`: `uv run python scripts/check_architecture.py`
  - `rust-lint`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
    Linux-only code (`cfg(target_os = "linux")`) is linted only in CI; read the
    log for those.
  - `migration-check`: `git fetch --no-tags origin main:refs/remotes/origin/main`,
    then `uv run python scripts/check_migrations.py` and
    `uv run python scripts/schema_page.py --check`
  - `macos / swift-test`: `cargo build -p loopflow --bin lf`, then
    `scripts/test_desktop.sh --filter '<failed suite>'`; also
    `uv run python scripts/check_desktop_launch.py` and
    `uv run python scripts/check_swift_multiplatform_boundaries.py`
  - `macos / loopflow-ui-test`: `uv run python scripts/test_ios.py` for the iOS
    half; implement reads `macos.yml:185-260` for the remaining steps and writes
    only the commands it ran.
  - `task-installation`: `uv run python scripts/test_task_installation.py`
    (CI passes `--container`; read `scripts/README.md` for the local form)
  - `e2e-smoke`: `uv run python scripts/test_network.py tests/e2e/test_smoke.sh`
    after building `target/debug/lf`
- **`documented_commands` does not read `.lf/skills/` or `scripts/`.** It scans
  `README.md`, `AGENTS.md`, `docs/`, builtins and top-level `skills/`
  (`rust/loopflow/tests/documented_commands.rs:78`). Only the `AGENTS.md` edit is
  in its reach. Every `lf` command quoted in an overlay is checked by hand with
  `lf help <path>`.
- **Display lock reads headless.** `ioreg -n Root -d1 -a | grep -A1 IOConsoleLocked`
  printed `<false/>` on this unlocked Machine. The `<true/>` reading was not
  observed in this run and cannot be produced from a headless step; the preflight
  says so and treats a missing key as unlocked.
- **`.lf/qa/desktop.md` already holds the locked-screen routes** (lines 151,
  195, 353, 453, 505: `-ui-test-mode live` snapshots, the `QAHook` file hook,
  `lf desktop state --json`) and what stays unreachable (Ghostty panes, menus,
  line 188). The preflight routes to them; it adds no new technique.
- **Anchors that move** with the Builds section: `rust/loopflow-execution/README.md:30`
  (`#builds`), `scripts/README.md:42`, `scripts/benchmarks/crate-rebuild/README.md:25,76`
  (`#shared-compilation-cache`). Relative links inside the moved text
  (`benchmarks/…`) keep working because the new file sits in `scripts/` too.
- `DESIGN.md`, `design/prototypes/`, `scripts/test_ios.py`,
  `swift/LoopflowTests/DTOFixtureTests.swift` exist.

## Design

### scripts/testing.md → three readers

| Reader | Gets |
|---|---|
| `implement` | inline in `.lf/skills/implement.md`; `docs:` line removed |
| `gate`, `ci-fix` | slimmed `scripts/testing.md` via `docs:` |
| anyone building or debugging the cache | `scripts/build_cache.md`, by link |

- `scripts/build_cache.md` takes lines 281–467 whole (`## Builds` and
  `### Shared compilation cache`), promoted one heading level. Nothing is
  rewritten; this is a move.
- `scripts/testing.md` keeps Plan first, Gate, macOS traps, Leave to CI,
  Boundary checks; gains a one-line link to `build_cache.md`; its opening line
  names `gate` and `ci-fix`.
- The "Implement: one focused test" section leaves `testing.md`. Its command
  block and "never the whole workspace" line move into `implement.md`; its
  decision-rows paragraph moves under Plan first (gate and ci-fix name those
  rows).
- `AGENTS.md:199-200` points at both files.

### .lf/skills/implement.md

Adds, above the existing UI rules:

- The focused-test block (library module, harness decoder, integration module,
  standalone binary, Python file), the never-`cargo test --all` line, and the
  `LF_*` unset + `< /dev/null` rule with the zsh form.
- Crate and surface map, one line each: `loopflow-model` (wire types; changes
  reach Swift), `loopflow-storage` (store, migrations), `loopflow-harness`
  (decoders, transcripts; builds no storage or execution), `loopflow-execution`
  (launch, skills, shipping), `loopflow` (CLI, server, integration tests),
  `swift/LoopflowDesktop`, `swift/LoopflowiOS`, `website/`. Implement confirms
  each directory name against the tree before writing it.
- **DTO parity**: a wire field changes `tests/fixtures/dto/`, the Rust and Swift
  required-nullable decoders, then `scripts/test_desktop.sh --filter DTOFixtureTests`
  and `uv run python scripts/test_ios.py`.
- **iPhone parity**: a change to shared Swift views, themes or Task status is
  checked in the iOS target too.

### .lf/skills/ci-fix.md (new)

`extends: builtin`, `docs: scripts/testing.md`. One "In this repository" section:
the check table from Findings (check, what it means, local command), the
`required-integration` rule, and the `LF_*`/stdin rule. Short; the builtin
already owns the procedure.

### .lf/skills/gate.md

The preparation block becomes:

```bash
lf commit -m "gate: checkpoint"
lf sync                              # last thing before publishing, after review fixes too
cargo fmt --check && cargo clippy --all-targets -- -D warnings   # when Rust changed
lf scratch clear
lf pr publish
lf pr checks
```

Plus: when `rust/loopflow-model/` or `tests/fixtures/dto/` changed, run
`scripts/test_desktop.sh --filter DTOFixtureTests` and
`uv run python scripts/test_ios.py` before publishing. `AGENTS.md`'s
"When to run what" gate line gains the lint so the two agree.

### .lf/skills/kickoff.md

Adds a blast-radius rule: name what the change does to each of CLI, server
RPC and schema, storage and migrations, DTOs and fixtures, Desktop, iPhone,
docs, or say it doesn't reach it. UI Tasks read `DESIGN.md` and
`design/prototypes/` first.

### .lf/skills/qa.md and .lf/qa/

- "Starting a Task on a private Machine" (qa.md:21-51) merges into
  `.lf/qa/served-launches.md`; its `Read when` line widens to cover starting a
  Task on a private Machine. "Shared build cache" (qa.md:53-64) merges into
  `.lf/qa/build-cache.md`, replacing its "see the qa skill's shared-cache notes"
  pointer. Every sentence survives; `qa.md` keeps one routing line for CLI and
  server changes.
- Display-lock preflight opens `qa.md`'s Desktop section and `.lf/qa/desktop.md`:
  the `ioreg` line, then the route. Locked: headless `scripts/test_desktop.sh`,
  the file hook and snapshot sections, `lf desktop state --json`; Ghostty panes,
  menus and keyboard are reported open, not retried. Unlocked: as today.

## Choices made for review

- **Lint runs only when Rust changed.** The brief says require fmt and clippy
  before publish; a prose-only branch (this one) would pay a clippy build for
  nothing. Passed over: unconditional lint.
- **Lint runs after `lf sync`, not before.** Main can add callers; linting the
  pre-sync tree repeats the leak.
- **One copy of the focused-test commands in skill docs.** They move to
  `implement.md` and leave `testing.md`; `AGENTS.md`'s own short block stays.
  Passed over: a copy in each.
- **The whole Builds section moves**, not only the cache subsection: the brief's
  188 lines are 281–467.

## Exclusions

- `AGENTS.md`'s 512 lines stay, apart from the two touched sentences.
- No builtin skill edits; no change to how `docs:` loads.
- `.lf/skills/hotfix.md` and the other overlays are untouched.
- No new lock-screen technique; no change to `QAHook.swift`.

## What goes

- `docs: scripts/testing.md` in `.lf/skills/implement.md`.
- `scripts/testing.md` lines 281–467 and its "Implement" section (moved).
- `.lf/skills/qa.md` lines 21–64 (moved).
- Must survive: every sentence of the moved sections; every existing link to
  `scripts/testing.md`; `docs/config.md` and `docs/orchestration.md` examples
  (`docs: [scripts/testing.md]` stays true for gate).

## One pass

Order: split `testing.md` and fix the four anchors first (the only step that
can break a link), then the overlays, then qa.

## Acceptance at gate

```bash
U=$(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /')
for s in implement gate ci-fix qa kickoff; do env ${=U} lf help $s < /dev/null | head -1; done
# five lines; ci-fix reads "skill (.lf/skills/ci-fix.md)"
! grep -q 'docs:' .lf/skills/implement.md
grep -rn 'testing.md#' --include='*.md' . | grep -v '^./\(target\|release\|\.lf/prompts\)'
# every anchor printed exists as a heading in scripts/testing.md
grep -rn 'build_cache.md' AGENTS.md scripts/testing.md scripts/README.md rust/loopflow-execution/README.md
env ${=U} cargo test -p loopflow --test integration documented_commands:: < /dev/null
wc -l scripts/testing.md .lf/skills/qa.md      # about 290 and about 25
```

## Demo

`lf help ci-fix` shows the repository's check table under the builtin
procedure, and `lf help implement` shows the commands and crate map with no
`scripts/testing.md` attached. Before and after: `wc -c scripts/testing.md`
(28 KB today) against what `implement` loads (0) and what `gate` loads.
