# 4fb4fe: the Antigravity agent is named `agy`

A person names the harness by its command, like the other four: `lf -a agy`,
`agents: [agy, claude]`, `agy:gemini-3.8-flash-high`,
`supported_harnesses: [..., agy]`. `antigravity` is refused wherever a harness
is named, with the usual "unsupported agent … choose claude, codex, opencode,
pi, or agy". #1998 is unreleased, so no store row, config file or migration
carries the old name; nothing to migrate.

Behavior of the harness does not change.

## Source of truth

`HarnessKind::as_str` / `HarnessKind::parse` in
`rust/loopflow-harness/src/lib.rs`. `Antigravity => "agy"`, and `parse` accepts
only `"agy"`. Every surface that prints or reads a harness name goes through
these: `Agent::parse`, settings validation, the `<harness>_error:` failure
prefix, `<harness>.stderr.log`, the hook script's `--harness` argument,
`lf task status` provider fields, Desktop's provider strings.

Type and module names keep the product name: `HarnessKind::Antigravity`,
`loopflow_harness::Antigravity`, `antigravity.rs`, `antigravity_decode`,
`transcript::antigravity`. Product facts stay too: `~/.gemini/antigravity-cli`,
`brew install --cask antigravity-cli`, prose "Google Antigravity CLI".

## What goes

`program()` exists only because `antigravity` ran `agy` (#1998 diff):

- `HarnessKind::program` and `Harness::program` (default + the `"agy"`
  override) in `loopflow-harness`. Callers go back to `kind().as_str()`, as
  before #1998:
  - `execution/harness/mod.rs` `Launcher::installed` and default `recognizes`
  - `execution/harness/native_session.rs` `Command::new`
  - `execution/harness/antigravity.rs:73` `Command::new`
  - default `Harness::recognizes_driver` → `command_program(command, self.kind().as_str())`
  - tests: `settings_launch_tests.rs:1418`, `task_graph_launch_tests.rs:2044,2227`
    (the `HarnessKind::parse(provider).program()` dance collapses back to
    `provider`), and the `parse("Antigravity").map(program) == Some("agy")`
    unit assertion in `lib.rs` tests becomes `parse("agy")`/`parse("antigravity") == None`.
- `Launcher::installed` stays (made for review): #1998 replaced
  `agent::check_cli_available(&str)` with it at four callers (`agent/mod.rs`,
  `config.rs`, `ops/recovery.rs`, `replay.rs`). It takes a `HarnessKind`
  instead of a free string, which is the better API whatever the command is
  called; restoring `check_cli_available` would bring back string plumbing.
  Its body becomes `which_on_path(Path::new(self.kind().as_str()))`.
- Literal `"antigravity"` passed where the kind's name is meant:
  `Stderr::pipe(..., "antigravity")` → `kind.as_str()`; the hook script's
  `--harness antigravity`; temp suffix `-antigravity` → `-agy`.

Stays, because it is the real process, not a name lf chose:
`Antigravity::recognizes_driver` and execution's `recognizes` still match both
`agy` and `antigravity` (`agy` execs a process named `antigravity`). Comments
there say so already.

## Surfaces to update

- Rust messages: `error.rs` (two harness lists), `config/settings.rs:289`,
  `config/catalog.rs` `yolo`/`chrome` help (the products are named there, so
  "Antigravity" prose can stay; only change if a harness *name* is listed).
  Install hint in execution `antigravity.rs:350` ("…; the command is `agy`.")
  drops the now-redundant clause: "Install it with
  `brew install --cask antigravity-cli`." `docs/troubleshooting.md:155`
  ("`antigravity` agent runs the `agy` command") goes the same way.
- Tests: `conformance_tests.rs` `"antigravity_error"` → `"agy_error"`;
  `launch_matrix.rs` (`harness: "antigravity"`, `Agent::parse("antigravity")`);
  execution `antigravity.rs` tests (`Agent::parse("antigravity:…")`,
  `--harness antigravity` expectations); `launch_policy_tests.rs:259`;
  `task_graph_launch_tests.rs` (`"antigravity", "antigravity"` rows of
  `a_task_step_runs_as_a_worker_on_every_harness` and the interrupt test);
  `steers_tests.rs` comment only.
- Stand-in: `rust/loopflow-execution/tests/support/antigravity_cli.py` →
  `agy_cli.py` (it stands in for the `agy` command); update the three
  `include_str!` sites.
- Swift: `SessionProgramStatus.swift:119` `case "agy": name = "Antigravity"`;
  `WorkspacePanesView.swift:597` picker list; `SessionsStoreTests.swift:12`
  arguments. The picker shows the agent name, so it reads `agy` like `pi`.
- Docs: `docs/config.md`, `docs/troubleshooting.md` (`antigravity_error:`,
  `antigravity.stderr.log`), `docs/security.md`, `docs/sessions.md`,
  `docs/telemetry.md`, `docs/getting-started.md`, `docs/index.md`,
  `docs/glossary.md`, `docs/agent-api.md`, `docs/dev/*`, `README.md`,
  harness READMEs (`execution/src/harness/README.md`,
  `loopflow-harness/README.md`), `rust/loopflow/tests/README.md`,
  `swift/README.md`, `skills/loopflow/README.md`, builtin
  `ops/skill/init.md`, `website/main.py`, `.lf/qa/antigravity.md` (commands
  become `-a agy`, `agy_error:`). Prose naming the product stays "Antigravity".
- DTO fixtures under `tests/fixtures/dto/`: none mention antigravity today
  (`git grep` found none); nothing to change.

Sweep at the end: `git grep -n -i '"antigravity"\|antigravity_error\|antigravity\.stderr\|-a antigravity\|antigravity:'`
should leave only the process-name match, the `~/.gemini/antigravity-cli`
path, the brew cask and product prose.

## Acceptance check at gate

- `cargo test -p loopflow-harness --lib` — `parse("agy")` is `Antigravity`,
  `parse("antigravity")` is `None`.
- `cargo test -p loopflow-execution --lib harness::` — conformance
  `agy_error` classification, launch matrix, antigravity unit tests on the
  `agy_cli.py` stand-in.
- `cargo test -p loopflow --lib a_task_step_runs_as_a_worker_on_every_harness`
  and `…on_antigravity` (worker on `agy`).
- By hand with a branch build: `lf -a antigravity default 'hi'` fails
  "unsupported agent: antigravity; choose claude, codex, opencode, pi, or agy";
  `lf set --global agents antigravity` is refused by settings validation.
- Swift view/unit tests run in CI (`SessionsStoreTests` with `agy`).

## Demo

`lf -a agy default 'say hi'` in a worktree with `agy` installed: a
conversation opens on Antigravity, `lf task status` shows provider `agy`, and
the Desktop pane label reads "Antigravity". `lf -a antigravity …` is refused
by name.

## Slices

1. **Rename at the source + delete `program()`.** `as_str`/`parse` →
   `agy`; remove both `program`s and restore `kind().as_str()` at callers;
   fix literal `"antigravity"` sites; rename the stand-in; update every Rust
   test. Run the three test commands above. This slice proves nothing else
   in the code keyed on the old string.
2. **Swift + docs + skills + website + qa note**, then the sweep grep.

## Conflicts

Task 4bd948 (yolo on by default) edits the same `yolo` help line in
`config/catalog.rs` and the docs around it; expect a small rebase conflict
there. Keep this change to the harness name in that line.
