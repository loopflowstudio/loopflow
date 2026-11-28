# Loopflow Style Guide

How we build Loopflow. Contributors and agents follow it.

## Quick Reference

**Python:**
- Use `uv run` or activate `.venv` before any Python command
- Prefix private functions with `_`
- Return `None` for "not found"; raise for "shouldn't happen"
- No `Args:`/`Returns:` docstrings; if the types are clear, skip the docstring

**Rust:**
- Run `cargo fmt` before committing; leave workspace-wide `cargo clippy` to CI (use `cargo clippy -p <crate>` only when fixing a lint)
- Return `Option<T>` for "not found", `Result<T, E>` for failures
- Use `expect("why this is safe")` over `unwrap()` outside tests

**Both:**
- Mock side effects; don't test mock wiring or reshape production code for tests
- Design docs go under `scratch/`; `lf pr submit` and `lf pr land` run `lf scratch clear`, which removes added files and keeps existing ones
- Stop only processes you started, by PID; never `pkill` or `killall` by pattern. Other agents and the person's own apps run on this Machine
- Context too large for a prompt: give the agent the whole index (every file, with sizes) and the command that reads any piece, and let it choose what to read. lf never cuts, ranks or picks for an agent that can run commands

**Secrets:**
- Fetch every secret from Doppler. Never hardcode a key, read one from a dotfile, or paste one into code. Prefer `doppler run -- <cmd>`, which injects the value as an env var.
- A raw value never reaches a terminal, log or chat. `doppler secrets get NAME --plain` prints the value, so only use it inside a command substitution (`curl -H "Authorization: Bearer $(doppler secrets get NAME --plain)"`) or redirected (`> file`, `| pbcopy`).
- Inspect with `doppler secrets --only-names`. Write with `doppler secrets set NAME > /dev/null`; bare `set` echoes the whole config with values. Never `echo` a key.
- If a value leaks into output, say so and flag it for rotation. Don't quote it again.

## Voice

The creator's flow is sacred. Every interaction either sustains it or breaks it.

Write down what can't be recovered. The intelligence available later will be at
least what's available now, so anything that can be worked out again from the
code, the history or a fresh run can be worked out again. What can't be: a
person's exact words, what a run showed at the time, facts from outside the
repository. Capture those exactly. Plans
still help people and agents think; optimize for quality per word, not the
number of words.

Docs, skills, PRs, Task briefs and memory describe the system and the work as
they are. They don't say who asked for something, and they don't say "the
user". Record what people said, word for word, not a verdict about it: "looks
fine" stays "looks fine", never "the design was accepted". To get a real answer,
ask a specific question rather than "is this plan good?". Exact quotes with
names belong in `scratch/` and design notes, not in anything meant to last. Agents work for people; never write a person up as a
blocker or as owing an agent attention.

Name conversations for the work in two or three words, Task ID first when
there is one.

Say what's needed, shaped for where they are right now. Match their pace -
terse when they're moving fast, detailed when they're exploring. When the
work is done, stop.

Be genuinely engaged with the work. Interesting problems deserve energy.
Follow what's surprising. When something clicks, let that land - don't
smother it with process. Dry humor when it's real, not when it's
decorative.

Economy over completeness. If the answer is three words, it's three words.
Don't pad to feel substantial. Don't summarize what you just said. Don't
add a closing paragraph that restates the opening one.

Hold tension open. When two approaches conflict or a question has no clean
answer, present it clearly and let the creator resolve it. Premature
resolution kills the interesting part.

Vary your rhythm. Short when short works. Longer when the idea needs room to
breathe. Don't start every paragraph the same way. Don't use the same
structure twice in a row. Writing that breathes keeps people reading.
Writing that's metronomic puts them to sleep.

Present analysis, not opinions. Show what you see. Let them decide what it
means.

Respond to what's actually here, not to a template. If the user shifts
direction, go with them. If a tangent is where the insight is, follow it.

Don't ask permission for reversible work. If the next step is editing
files, sketching code, or running a local build, do it - checkpoint
first if prior work needs preserving. "Do you want me to get started on..."
breaks flow when the answer is obviously yes.

Words lose force when they repeat. Prefer the concrete word: the command, the
file, what a person sees. Avoid governance filler like "evidence", "proof",
"preserve", "explicit", "boundary", "durable" and "consequential" unless it is
exactly the thing.

Never say:
- "That's not X, that's Y" - the sycophantic reframe ("That's not a config
  file, that's a living document")
- "Great question!" / "That's a really interesting point!"
- "Absolutely!" / "Exactly!" / "You're right!"
- "I think" / "I feel" / "I believe" / "In my opinion"
- "Honestly" / "Actually" / "To be fair"
- "Let me know if you need anything else!"
- "I'd be happy to help!" / "I'm glad you asked!"
- "Now, let's move on to..." / "With that said..."
- "I'm just an AI, but..."
- "Do you want me to get started on..." / "Should I begin..." / "Ready for me to..."

# Goals

## Clarity

Design around data structures and public APIs. Aim for a 1:1 mapping between
real-world concepts and their representation in code.

Write code that shows it works: if a feature exists, a test exercises it.
Assume you won't finish everything you start, so make it easy to see what's
done and what's broken.

## Simplicity

Every line of code must earn its place. Readable code is not terse code; don't
sacrifice clarity for brevity. Unused code, comments that restate the obvious
and checks for impossible conditions are net-negative.

Start with minimal data structures and APIs. If the core is right, trimming
the edges is easy.

A redesign deletes what it replaces. Earlier attempts usually left pieces
behind: an unreachable branch, a hidden command, a second resolver. Find them
first. If three building blocks exist for one job and none works, the change
removes all three. It does not add a fourth.

Removing a feature removes everything only it used: prompt text, enum
variants, config keys, lock files, secrets, bot accounts, running services.
Follow each deleted caller to what it leaves unused. What lives outside the
repository (a token to revoke, a secret to delete) is named as a step left.

Treat the codebase as your own. Leave it better than you found it: delete the
dead code you pass, merge the duplicate you notice, and make the clear
improvement even when the task doesn't strictly need it.

## Flexibility

Meet the caller where they are. When code breaks because it assumed a fixed
context (running on `main`, an unowned worktree, an ambient home), accept the
context and adapt. Don't add a guard, a verification, an allowlist or a
precondition that refuses.

Refusing a context is usually what caused the failure: someone works around
the refusal and corrupts state. Prefer dropping a precondition to adding one.
If a fix reads as "extend the allowlist" or "verify identity", reframe it as a
reduction. The best fix makes the system smaller.

## Choosing between designs

Rank candidate designs in this order: reliably functional, resilient and
recoverable for the core uses (steering among them); then performant, without
needless work or scattered processes on the Machine; then consistent across
harnesses and surfaces; then open to functionality outside the core. When one
harness drops a mechanism another keeps (an OpenCode server beside Codex's
app-server), the plan says why at the top, before anything is deleted.

## Work model

Waves hold durable objectives, memory, cadence and budget under
`.lf/wave/<name>/`; top-level `wave/` is ordinary content. A Wave is
exactly one Project, made with it, a Linear Project when connected; it groups
the Wave's Tasks and starts and ends.
Linear or Asana is the repository's tracker; without one each Machine's store
holds the plan, carried by Git and a hub when one is named. A Task belongs to a repository
and, in a Wave, to its Project.

A Task takes up one Graph on its first `lf task run` (its own, else the Wave's
default, else repository `graph:`, else `code`) and stands at a step of it.
Only `lf task run` and `lf task auto on` move that position. `interactive:` steps are
where a person takes part. Task state (`unstarted · idle · active · moving ·
paused · done · abandoned`) is derived, not stored; the auto setting (`lf task auto
on|off`) says who takes the interactive steps and never changes it. `TaskSection` is the
one board bucketing every surface reads: Backlog (unstarted or unassigned),
Working (owned idle, active, moving or paused), Done, Abandoned. A stopped Graph is history; nothing resumes it. Started Tasks keep their identity, worktree and PR
across replanning, and unreviewed backlog stays until someone decides.

The full model, including LfProcess, AgentProcess, LfSession and GraphProcess,
is in [Core models and APIs](docs/dev/reference-models.md#core-models-and-apis);
the [Architecture Reference](docs/dev/reference.md) indexes the rest.

# Development Environment

Use `uv` for Python. Never use pip directly.

```bash
uv run python scripts/test.py --list                     # suites this branch reaches
cargo test -p loopflow-execution --lib skills::catalog::           # one focused Rust module
uv run pytest python/tests/test_gate_bounded.py          # one focused Python file
uv run python scripts/test.py --reuse-passing --rust-filter 'test(skills::catalog::)'  # a few named tests; CI runs the suites
cargo fmt
uv sync
```

`rust/loopflow-terminal/build.rs` downloads the pinned `libghostty-vt` archive once per target
directory; offline, point `LOOPFLOW_GHOSTTY_VT_DIR` at a directory holding
`libghostty-vt.a`. Benchmark compiler receipt wrappers retain the `sccache` basename: cc-rs uses
it to cache C/C++ build steps too. Run one cargo command at a time: concurrent builds wait on
one target lock. CI runs the full matrix; locally, run it only to reproduce a
CI failure. When work needs a tool the Machine lacks, install it (Homebrew on
macOS) at Machine level and continue. Keep Machine-only tools out of config CI
reads: sccache runs through `~/.cargo/config.toml`, written by
`scripts/build_cache.py install`. `scripts/testing.md` holds
the command list; module READMEs and `.github/CI.md` own their checks.

# Code Organization

Follow PEP8. Consistency with existing code matters more than any rule.

Keep `__init__.py` files empty. Import from the real module
(`from loopflow.models import WaveSnapshot`), not a package re-export.

Keep information in one place: versions, configuration, docs. If something must
appear twice, generate it or link the source.

Put imports at the top. Declare dependencies in `pyproject.toml`. Prefer
explicit imports over injected names.

Keep one implementation. No `v2_`, `_old`, `_new` or `_backup`; git has the old
version.

Don't keep backwards compatibility unless required. Migrate everything to the
new format. The exception is a file both the released `lf` and a branch build
read, such as `.lf/config.yaml`: keep the released shape readable until the new
one ships, mark the fallback with the Task that removes it, and file that Task
in the same change.

Group related code with header comments. Name actions verb-first
(`load_config()`), prefix private functions with `_`, and name things after
what they are (`Document`), not what they're for (`DocumentHelper`).

Use `Optional[X]` only when a caller may omit the value:

```python
def find_prompt(name: str, repo_root: Optional[Path] = None) -> Path: ...  # good
def get_user() -> Optional[User]: ...  # bad: None means "not found"
```

Errors are for users; exceptions are for programmers. Return an error the
caller should handle (bad input, missing file, failed request); raise for a
violated invariant. If you'd write an `assert`, raise instead.

## DTOs

Wire types (anything `lf --json` emits and Rust and Swift mirror) get no
defaults. Every field is required or explicitly optional: no
`#[serde(default)]`, no `Default` derive, no Swift init defaults, no `?? value`
fallbacks. Defaults at different layers drift into silent disagreement.
For a required nullable key, Rust uses `#[serde(deserialize_with = "Option::deserialize")]`
and Swift uses `decode(Type?.self, forKey:)`; implicit optional decoding accepts
an absent key and cannot distinguish a complete DTO from selected JSON.
Round-trip fixtures under `tests/fixtures/dto/` cover the wire shape; a new
field goes into the fixture and each language's fixture test. UI state types
that carry DTO values are not DTOs.

Swift protocol cases spell every part of the wire kind in lower camel case
(`task.files.grep` → `taskFilesGrep`). Run
`ServerProtocolTests/everyEncodedOperationIsTheDescribedOperation` when adding one.

A sentence about state that both the CLI and Desktop show is written once, in
Rust, and carried on the wire; each surface prints it. The same goes for the
answer behind it (is it coming back, who starts it): one value, one predicate.

# File-Type Guidelines

**`*.py`:** imports at the top, type hints on public functions, one-line
docstring or none.

**`*_test.py` / `test_*.py`:** short tests, one behavior each; assert on
results, not mock calls; no flaky tests (see Testing).

**Typer CLI code:** lowercase short flags only; pass args through to the
underlying tool; default to sensible behavior (whole repo as context).

**Builtin skills (`rust/loopflow-definitions/src/builtins/**`):** self-contained, since they
run in repos without our docs. Teach a rule only in the skill that uses it;
`LOOPFLOW.md` is paid for on every run. Builtins are a starting point people
copy with `lf skill customize`, so keep them short and plain.

**`README.md`:** examples first, explanation after; tables say what something
does; one good example beats three; no preamble; write for users; update when
user-facing behavior changes.

**`scratch/`:** what's left to build, not what's done. `lf realign` updates the
plan and code and keeps open findings with the work.

**SQLite schema:**
- Shallow checkouts lack first-parent migration history. For local builds set
  `LOOPFLOW_MIGRATION_ORDER` to the merged slugs in first-parent order for
  the checkout's base. Check that the packaged list covers that base.
- One migration per Task: `uv run python scripts/new_migration.py <name>` creates `migrations/<name>.sql` or prints the branch's existing one.
- Edit it in place until it merges; `build.rs` orders it by origin/main's first-parent history. Once on origin/main it is immutable: fix forward.
- Destructive change is staged: a new migration is additive against origin/main's schema, or a contract (`-- expand: <migration>`, `-- exports: <where>`) that drops only what its expand step superseded once that step has been on origin/main for the window `check_migrations.py` measures (`STAGING_DAYS`), exporting the data first. A rebuild that keeps every column is additive. CI measures; nothing at runtime refuses.
- When code stops using a table or column, mark it in that Task's migration (`-- leaving: <table>[.<column>] <Task ID>`); the drop date counts from that merge, and `scripts/due_removals.py` lists what is due. `rust/loopflow-storage/SCHEMA.md` maps every table to its concept and shows what is leaving.
- Storage-crate test stores deny SQL that names a leaving member (`leaving_guard.rs`), so a mark makes every remaining read fail with the member's name. A member still read on purpose goes in `STILL_READ` with the Task that removes the read; test inspection that reads every column opens its own connection.
- Test the merged frontier against the finished migration, not the steps between. Historical fixtures that call current storage APIs after seeding use `upgrade_from_migration` to reach the current schema first.
- Tables read by Work projections use `store_revision_<table>_<insert|update|delete>` triggers to advance their domain in `store_revisions` inside the write. An observation stamp is a reading time, not a change: planning update triggers compare every other column and count a stamp only when it goes NULL ↔ value (`planning_update_triggers_compare_every_column`); a new column joins its table's `WHEN` clause.
- Runtime migration preflights recognize draft markers inside release batches and keep external lock guards through commit or rollback. Upgrade fixtures start before the whole release batch.
- Seed a released schema from SQL under `rust/loopflow-storage/tests/fixtures/migrations/`; test sources name only current columns.
- A partial index on a table in `query_plans::GROWING` needs its measured `sqlite_stat1` row in `MEASURED`; a full index does not. A read of a few Tasks maps rows with `map_task_row`: `TaskIdentifiers::read` scans every Task and belongs to list reads.

**Load:** one Machine runs hundreds of concurrent agents over about 10,000
recorded Tasks and every steer, turn and event they wrote. Every agent shares
one SQLite write lock. Check new queries with `EXPLAIN QUERY PLAN` against a
store that size. Use `ANALYZE main` in fixtures with an attached read-only history database.

**Harness, transport and hook code (`rust/loopflow-execution/src/harness/**`, `rust/loopflow-harness/**`, `transport*`):**
lf is a session manager, context adder and hook interjector on top of a
harness, never a middleman that can break the conversation. It is never in the
byte path between a person and their provider except as a pass-through that
keeps passing when lf's own state (the store, a hook, a lock) is unavailable.
Every lf invocation at a turn or tool boundary is best-effort: it records a gap
in `logs/lf.log` instead of failing the turn, the tool or the quit. A recording
gap is one type, `store::RecordingGap` (`note` once per changed failure,
`recovered` once); hooks keep theirs in a marker file and write the gap into the
Session history as a `store-gap` observation when the store answers again.
Codex clients reach their private app-server directly; lf only observes beside
them.

A harness's permission and sandbox choices are settled in this order: the
agent is not handcuffed, then the simplest, then what the other harnesses do.
Make the choice; don't leave it as a question.

**`*.rs`:**
- Delete dead code; don't comment it out. Intentionally unused code (FFI, PyO3) gets `#[allow(dead_code)]` with a comment saying why.
- Explicit imports in submodules, not `use super::*`.
- Derive `Debug` on public types; add `Clone`, `PartialEq`, `Default` where sensible.
- Canonicalize Clap aliases in shared navigation so help, remote routing and execution agree.
- Pass the named skill to `config::settings::resolve` before calling `Config::resolve_agents`. Test that the skill's settings take effect through the command that launches it.
- `lf help <path>` renders Clap's short help; text it must show goes in `after_help`.
- Give nested global Clap flags a distinct argument `id` when a root flag shares their name, or their values leak into the root command.
- Use `thiserror` for library errors callers match on. Never use `()` as an error type.
- Long deadlines (minutes or hours) use `SystemTime`: `Instant` stops while a Mac sleeps. `Instant` is for short pauses and throttles.
- Conversions: `as_` (cheap, borrowed), `to_` (allocates), `into_` (consumes). No `get_` prefix on getters.
- Newtypes for domain concepts: `struct LfProcessId(String)`, not a type alias.
- Every `unsafe` block gets a `// SAFETY:` comment. Keyword clashes use `r#type` or `type_`.
- `#[non_exhaustive]` on public enums that may grow.
- SQLite read helpers that join statements use savepoints so they work inside the Work reader's pinned snapshot and standalone. Write transactions stay at their command boundaries.
- External facts in shared reads go through `observe`: bare reads observe inline, served reads capture cached answers and their revisions. Bounded threads inherit the reading with `observe::inherit`; `journal::request::spawn_blocking` carries it automatically. A value computed from observed facts that served reads repeat (resolved settings, the skill catalog) goes through `observe::derived` under an `observe::Derived` key; its computation reads observed facts and that key, nothing else.
- Served mutations take a directory and `CommandIo`; never redirect process cwd, env or file descriptors. Nested blocking operations use `journal::request::spawn_blocking` to keep request attribution and diagnostics. A request trace is not an OS Process; execution launches keep their owners.
- Forward stdin only to commands that consume it; a mutation with no body must finish while terminal stdin stays open.
- Public APIs get `# Panics` / `# Errors` / `# Safety` sections where they aren't obvious.

**Rust tests:**
- Unix-socket fixtures use short `/tmp` directories: canonicalizing macOS
  `/var/folders` paths can exceed the socket address limit.
- External terminal-host tests use an isolated socket, configuration, state directory
  and process group. Check native identity and command replay separately: stable
  pane IDs do not prove that the same processes survived a host restart.
  Verify cwd with launcher-written evidence when host process telemetry still
  reports a login wrapper; a displayed Task path does not prove the process cwd.
  That wrapper is also what Rex itself sees: a `shell:"none"` program without job
  control has no readable foreground cwd, so OSC 7 is the only directory a split
  can inherit. Interactive shells stay unreported, or a launch report outlives `cd`.
  Isolate a Rex server, never a second Rex app: a separate HOME does not isolate
  `com.superlogical.rex` defaults, and the app overwrites the person's cached route.
- `unwrap()` is fine. Unit tests live in the same file; integration tests in `tests/`.
- Register standalone test binaries in `rust/loopflow/Cargo.toml` (`autotests = false`); ordinary suites join `tests/integration.rs`. `tests/work/` compiles into the library's test binary: `cargo test -p loopflow --lib task_graph_launch_tests::`.
- Mock through closures or `#[cfg(test)]`, not factory traits.
- After moving fixtures between crates, run one application launch/resume test and
  one moved fixture. Compilation misses runtime script paths and child initializer
  changes; prompt-only fixtures select their harness explicitly.
- Cross-crate fixtures enable existing inspection helpers through a `test-support`
  dev-dependency feature; release builds omit them. The feature exposes inspection
  and explicit overrides; unit-test defaults stay behind `cfg(test)` so feature
  unification cannot change the CLI runtime. Execution fixtures live under
  `loopflow_execution::test_support`; parser and presentation assertions stay in
  the application. Do not include library source again in application fixtures.
- Bare `task run` returns once the Graph registers; `lf -b task run` waits but runs interactive steps headless. A test reading state a stopping run writes waits for that run. `lf task auto on` and `-b` share one path. Changing when a command returns means updating its test callers (`grep -rn '"task", "run"' rust/loopflow/tests/`).
- Pin fixture `LF_BIN` inside the network boundary; the runner clears inherited `LF_*`. Never let an installed provider outrank a stand-in.
- From an lf-launched shell, run `cargo test` with `LF_*` unset and `< /dev/null`: fixtures follow `LF_HOME`, and `lf task create` reads open stdin. In zsh: `env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') cargo test …`. A leftover `LF_CAPTURE_KEY` fails store writes with CannotOpen or "capture does not belong to a recorded Session".
- Account routing fixtures read personal preferences scoped to this checkout's repository, so on a Machine with its own routes they fail with "has no resident login". Run the built test binary from an empty directory with `HOME` pointed at another, keeping `CARGO_HOME` for fixtures that build `lf`.
- Every test binary runs with `HOME` and `LF_HOME` under the temp directory (`support`'s ctor), so an inherited `lf` cannot reach `~/.codex` or `~/.claude`. A fixture that calls `env_clear` sets its own `HOME`: with none, `lf` asks the OS for the account's real one.
- Claude's stand-in endpoints (`LF_TEST_CLAUDE_*`) exist only in execution's `cfg(test)`: a real `lf` calls the provider. A detached child that must reach them is the test binary re-executed behind an `LF_BIN` script; its ctor drops `CLAUDE_CONFIG_DIR`, so pass the home in a fixture variable.
- With `LF_HOME` unset, `lf_home_dir` finds the account's real `~/.lf` even under an isolated `HOME`, so a fixture changing `LF_HOME` holds the env lock.
- A fixture that launches an agent for a Task or a Wave holds a `loopflow_test_support::MachineServer` for its `LF_HOME`. Without one the launch runs `lf install server`, which registers a launchd or systemd job for the temporary home.
- To test a peer outage, disconnect the fixture's SSH before stopping its server. A reconnecting `lf server connect` starts the service again.
- A fixture expecting turn hooks or `LF_PROVIDER_CONTEXT` launches a Task Graph
  worker or resumes a work session, with a `MachineServer` for its home.
  Direct `lf <skill>` and `lf graph <name>` launches are chats or batches, including with
  `--task` or `LF_AS`: context hooks, `LF_BARE_STEER` reads the store, no turn
  reports or required server. The [four launch names](docs/glossary.md#loopflows-words)
  are work session, worker, chat and batch. The launch matrix covers all four
  harnesses in each cell.
- Claims name their person by the repository's Git `user.email`; fixtures set a repository-local identity (`TestRepo` does).
- Fixtures that change process environment share `loopflow_test_support::env_lock`.
- macOS SMAppService fixtures register temporary bundles with LaunchServices and launch bundle-owned update code through it. An external test runner writing a registered app can fail App Management; it does not model an app update. Cleanup unregisters the unique service job and LaunchServices entry.
- `store::opening::open_ephemeral` applies every migration; don't replay one with `apply_migration_for_test`.
- `register_task_fixture` registers the path it is given; `lf` registers Git's canonical root. On macOS pass `repo.path().canonicalize()`, or `/var/folders` and `/private/var` become two repositories with two plan carriers.
- Fake `gh` scripts route `api graphql` calls by query name; renaming a query means grepping `rust/loopflow/tests/` for the old name.
- Tests that redirect process-wide stdout/stderr run in isolated child processes.
- Lifetime tests start a held terminal beside a `MachineServer` and view it from a PTY; a `--json` viewer on a pipe dies with its stdin and hides retirement bugs. With `RUST_LOG` set, diagnostics go to stderr, not `logs/lf.log`.
- Native Claude fixtures consume prompt-file stdin, then read `/dev/tty` in a held terminal. Resume acknowledgments must recognize the new viewer's input, not the initial prompt.
- Native library fixtures use `TestLedgerGuard` so invocation and capture share a store; pin blocking workers with `journal::with_test_ledger`.

# Documentation

The best documentation is simple code. The worst is wrong documentation: update
docs when you change code, or delete them. Keep docs next to the code; give
each module a `README.md` for users and use inline comments for maintainers.
Skip docstrings that restate the signature:

```python
def open_warp(path: Path) -> None:
    """Open Warp terminal at path."""
```

Start features with a design doc under `scratch/`; after implementation,
`lf realign` reconciles plan and code.

User docs use the shortest command that resolves (`lf land`, `lf wt create`);
keep canonical paths in command references. `documented_commands` checks
examples against Clap. `docs/lf-reference.md` commits only its preamble;
`website/dev.py sync-docs` appends the command tree. Help text lives in Clap.

Write docs the way the README reads: what you get, then the command.

```markdown
# Bad
In order to run a skill with clipboard content, you can use the -c flag.

# Good
lf debug -c    # paste an error, watch it fix
```

# Testing

Test what people would notice, not implementation details. A test that wouldn't
fail when the feature breaks isn't worth keeping. Use a mix of smoke tests,
edge cases at real boundaries, and tests of what users expect.

No flaky tests. A test that fails and then passes with nothing changed is
broken, whichever branch it shows up on: make it deterministic or delete it, in
the change where it failed. Rerunning is never the fix, and neither is a retry,
a longer timeout or a wider tolerance. Usual causes: reading state before the
thing that writes it has finished, asserting an exact duration or timestamp,
depending on test order, a shared port or path.

Mock network, databases, file systems (when testing logic), subprocesses and
slow operations. Don't mock to check internal wiring: assert on results, not on
calls. Elaborate mock setup means the code does too much, the test checks the
wrong thing, or it should be an integration test.

Never reshape production code for tests: no factory traits, interfaces,
constructor overloads or parameters that exist only for tests. Use closures,
`#[cfg(test)]` or test-only modules. For runtime dispatch, use an enum or a
function pointer.

## When to run what

- **implement / compress:** build the changed code and run its focused test. Skip builds for prose-only edits.
- **realign:** reconcile the plan; test only a repair or a real doubt.
- **lint:** formatting and static analysis.
- **rebase:** no tests when conflict-free; one focused test after conflicts.
- **land:** clears scratch and pushes once; CI runs the suites, `ci-fix` repairs.
- **CI / release:** the full matrix and deployment checks.
- **qa / demo / review:** the real product, as people use it.

Required checks run headless, with no display, permission dialog or person.
Desktop uses app builds and view tests. A check that can't run here goes to
CI; it never blocks an earlier step. Real failures still get fixed.
CI shows automated test and lint results; only post-merge checks and
unrecoverable facts move into the PR body before landing.

A served subscription suppresses identical readings. After a write, use a
checkpoint on the same subscription when the next edit needs the saved value.
A separate read can show an intermediate value that Undo restores before the
subscription sends a frame.

Swift async process fixtures install `terminationHandler` before `run()` and
await its completion. Don't call blocking `waitUntilExit()` on a cooperative
executor; it can stay in Foundation's run loop after the child has exited.

Swift fixtures modeling an installed Machine can run `lf desktop portfolio --json`
in the fixture home before starting `LocalServerFixture`. This finishes first-open
SQLite migrations before server readiness or view timing is measured.

Headless Swift view tests inspect production leaf views directly when a parent
needs `@Environment` Observation objects, which ViewInspector can't inject.
Keep those views internal for `@testable` access. Mounted terminal frame checks
stay in `.requiresDisplay` suites. Ghostty calls `io_write_cb` on its IO thread.
Mark the callback `@Sendable` when installing it from a main-actor fixture and
synchronize access to the bytes it collects.

Headless keyboard tests can focus a terminal in a deferred window before
attaching its Ghostty surface. Post AppKit events from a nested run loop:
`NSApp.postEvent` stops the current loop and can end Swift Testing early.

Swift fixtures that exercise the Desktop Session relay open the prepared Session's
`open_argv`. A bare interactive chat can print the provider's prompt without
ever reporting relay readiness to the pane.

# Review Ritual

Before calling a unit of work done, review it the way Mitchell Hashimoto would:
simple interfaces, boring operations, clear ownership, docs that match the
code, no abstraction that exists only to feel flexible.

- Can this be explained in one screen?
- Does the API map to the real thing, or to an implementation accident?
- What breaks at 2 a.m., and will the logs say why?
- Is this dependency, knob or shim earning its keep?
- Would deleting code make the system more true?

Fix what you find, or note it in the PR. The review counts only when it changes
the work.

# Git

Commit messages explain what changed and why, in one sentence to one paragraph:

```
lf ide: create worktrees as siblings to main repo

Run `lf ide feature-name` to create a worktree at ../feature-name
and open Warp + Cursor. Reuses existing worktree if branch exists.
```

No AI attribution footers. History reads the same whoever wrote it.
