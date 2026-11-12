# Loopflow Style Guide

How we build Loopflow. Contributors and agents follow it.

## Quick Reference

**Python:**
- Use `uv run` or activate `.venv` before any Python command
- Prefix private functions with `_`
- Return `None` for "not found"; raise for "shouldn't happen"
- No `Args:`/`Returns:` docstrings; if the types are clear, skip the docstring

**Rust:**
- Run `cargo fmt` and `cargo clippy --all-targets -- -D warnings` before committing; CI treats warnings as errors
- Return `Option<T>` for "not found", `Result<T, E>` for failures
- Use `expect("why this is safe")` over `unwrap()` outside tests
- Derive `Debug` on all public types

**Both:**
- Mock side effects; don't test mock wiring or reshape production code for tests
- Design docs go under `scratch/`; `lf pr submit` and `lf pr land` run `lf scratch clear`, which removes added files and keeps existing ones
- Headless runs make the call and keep moving; note genuinely open choices in `scratch/questions.md`

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

## Flexibility

Meet the caller where they are. When code breaks because it assumed a fixed
context (running on `main`, an unowned worktree, an ambient home), accept the
context and adapt. Don't add a guard, a verification, an allowlist or a
precondition that refuses.

Refusing a context is usually what caused the failure: someone works around
the refusal and corrupts state. Prefer dropping a precondition to adding one.
If a fix reads as "extend the allowlist" or "verify identity", reframe it as a
reduction. The best fix makes the system smaller.

## Work model

Waves hold durable objectives, memory, cadence and budget under
`.lf/wave/<name>/`; top-level `wave/` is ordinary content. A Wave's current
plan is its In Progress Linear Project, which holds Tasks, KRs and targets;
"Chapter" is just that Project's shared name. A Task belongs to a repository
and optionally a Project.

A Task takes up one Graph on its first `lf task run` (its own, else the Wave's
default, else repository `graph:`, else `code`) and stands at a step of it.
Only `lf task run` and `lf ship` move that position. `interactive:` steps are
where a person takes part. Task state (`unstarted · idle · active · moving ·
shipping · done · abandoned`) is derived, not stored. `TaskSection` is the
one board bucketing every surface reads: Backlog (unstarted or unassigned),
Working (owned idle, active or moving), Autonomous (owned shipping), Done,
Abandoned. A stopped Graph is history; nothing resumes it. Started Tasks keep their identity, worktree and PR
across replanning, and unreviewed backlog stays until someone decides.

The full model, including LfProcess, AgentProcess, LfSession and GraphProcess,
is in [Core models and APIs](docs/dev/reference-models.md#core-models-and-apis);
the [Architecture Reference](docs/dev/reference.md) indexes the rest.

# Development Environment

Use `uv` for Python. Never use pip directly.

```bash
uv run python scripts/test.py --list                     # suites this branch reaches
cargo test -p loopflow --lib skills::catalog::           # one focused Rust module
uv run pytest python/tests/test_gate_bounded.py          # one focused Python file
uv run python scripts/test.py --reuse-passing --rust-filter 'test(skills::catalog::)'  # gate: affected suites once per tree
cargo fmt
cargo clippy --all-targets -- -D warnings
uv sync
```

`build.rs` downloads the pinned `libghostty-vt` archive once per target
directory; offline, point `LOOPFLOW_GHOSTTY_VT_DIR` at a directory holding
`libghostty-vt.a`. Run one cargo command at a time: concurrent builds wait on
one target lock. CI runs the full matrix; locally, run it only to reproduce a
CI failure. When work needs a tool the Machine lacks, install it (Homebrew on
macOS) at Machine level and continue. Keep Machine-only tools out of config CI
reads: sccache runs through `~/.cargo/config.toml`. `scripts/testing.md` holds
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

# File-Type Guidelines

**`*.py`:** imports at the top, type hints on public functions, one-line
docstring or none.

**`*_test.py` / `test_*.py`:** short tests, one behavior each; assert on
results, not mock calls; delete flaky tests rather than adding retries.

**Typer CLI code:** lowercase short flags only; pass args through to the
underlying tool; default to sensible behavior (whole repo as context).

**Builtin skills (`rust/loopflow/src/builtins/**`):** self-contained, since they
run in repos without our docs. Teach a rule only in the skill that uses it;
`LOOPFLOW.md` is paid for on every run. Builtins are a starting point people
copy with `lf skill customize`, so keep them short and plain.

**`README.md`:** examples first, explanation after; tables say what something
does; one good example beats three; no preamble; write for users; update when
user-facing behavior changes.

**`scratch/`:** what's left to build, not what's done. `lf realign` updates the
plan, code and memory and keeps open findings with the work.

**SQLite schema:**
- One migration per Task: `uv run python scripts/new_migration.py <name>` creates `migrations/<name>.sql` or prints the branch's existing one.
- Edit it in place until it merges; `build.rs` orders it by origin/main's first-parent history. Once on origin/main it is immutable: fix forward.
- Destructive change is staged: a new migration is additive against origin/main's schema, or a contract (`-- expand: <migration>`, `-- exports: <where>`) that drops only what its expand step superseded once that step has been on origin/main for the window `check_migrations.py` measures (`STAGING_DAYS`), exporting the data first. A rebuild that keeps every column is additive. CI measures; nothing at runtime refuses.
- Test the merged frontier against the finished migration, not the steps between.
- Tables read by Work projections use `store_revision_<table>_<insert|update|delete>` triggers to advance their domain in `store_revisions` inside the write.
- Runtime migration preflights recognize draft markers inside release batches and keep external lock guards through commit or rollback. Upgrade fixtures start before the whole release batch.
- Seed a released schema from SQL under `rust/loopflow/tests/fixtures/migrations/`; test sources name only current columns.

**Load:** one Machine runs hundreds of concurrent agents over about 10,000
recorded Tasks and every steer, turn and event they wrote. Every agent shares
one SQLite write lock. Check new queries with `EXPLAIN QUERY PLAN` against a
store that size.

**Harness, transport and hook code (`rust/loopflow/src/harness/**`, `transport*`):**
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

**`*.rs`:**
- Delete dead code; don't comment it out. Intentionally unused code (FFI, PyO3) gets `#[allow(dead_code)]` with a comment saying why.
- Explicit imports in submodules, not `use super::*`.
- Derive `Debug` on public types; add `Clone`, `PartialEq`, `Default` where sensible.
- Canonicalize Clap aliases in shared navigation so help, remote routing and execution agree.
- `lf help <path>` renders Clap's short help; text it must show goes in `after_help`.
- Give nested global Clap flags a distinct argument `id` when a root flag shares their name, or their values leak into the root command.
- Use `thiserror` for library errors callers match on. Never use `()` as an error type.
- Long deadlines (minutes or hours) use `SystemTime`: `Instant` stops while a Mac sleeps. `Instant` is for short pauses and throttles.
- Conversions: `as_` (cheap, borrowed), `to_` (allocates), `into_` (consumes). No `get_` prefix on getters.
- Newtypes for domain concepts: `struct LfProcessId(String)`, not a type alias.
- Every `unsafe` block gets a `// SAFETY:` comment. Keyword clashes use `r#type` or `type_`.
- `#[non_exhaustive]` on public enums that may grow.
- SQLite read helpers that join statements use savepoints so they work inside the Work reader's pinned snapshot and standalone. Write transactions stay at their command boundaries.
- External facts in shared reads go through `observe`: bare reads observe inline, served reads capture cached answers and their revisions. Bounded threads inherit the reading with `observe::inherit`; `journal::request::spawn_blocking` carries it automatically.
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
- Register standalone test binaries in `rust/loopflow/Cargo.toml` (`autotests = false`); ordinary suites join `tests/integration.rs`. `tests/linear/` compiles into the library's test binary: `cargo test -p loopflow --lib task_graph_launch_tests::`.
- Mock through closures or `#[cfg(test)]`, not factory traits.
- Bare `task run` returns once the Graph registers; `lf -b task run` waits but runs interactive steps headless. A test reading state a stopping run writes waits for that run. `lf ship` and `-b` share one path. Changing when a command returns means updating its test callers (`grep -rn '"task", "run"' rust/loopflow/tests/`).
- Pin fixture `LF_BIN` inside the network boundary; the runner clears inherited `LF_*`. Never let an installed provider outrank a stand-in.
- From an lf-launched shell, run `cargo test` with `LF_*` unset and `< /dev/null`: fixtures follow `LF_HOME`, and `lf task create` reads open stdin. In zsh: `env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') cargo test …`. A leftover `LF_CAPTURE_KEY` fails store writes with CannotOpen or "capture does not belong to a recorded Session".
- Account routing fixtures read personal preferences scoped to this checkout's repository, so on a Machine with its own routes they fail with "has no resident login". Run the built test binary from an empty directory with `HOME` pointed at another, keeping `CARGO_HOME` for fixtures that build `lf`.
- Every test binary runs with `HOME` under the temp directory (`support`'s ctor), so an inherited `lf` cannot reach `~/.codex` or `~/.claude`. A fixture that calls `env_clear` sets its own `HOME`: with none, `lf` asks the OS for the account's real one.
- Claude's stand-in endpoints (`LF_TEST_CLAUDE_*`) exist only under `cfg(test)`: a real `lf` calls the provider. A detached child that must reach them is the test binary re-executed behind an `LF_BIN` script; its ctor drops `CLAUDE_CONFIG_DIR`, so pass the home in a fixture variable.
- With `LF_HOME` unset, `lf_home_dir` finds the account's real `~/.lf` even under an isolated `HOME`, so a fixture changing `LF_HOME` holds the env lock.
- A fixture that launches an agent for a Task or a Wave holds a `loopflow_test_support::MachineServer` for its `LF_HOME`. Without one the launch runs `lf install server`, which registers a launchd or systemd job for the temporary home.
- A headless fixture that expects steers, turn hooks or `LF_PROVIDER_CONTEXT` launches managed: its command names the Task (`--task`), with a connected fixture account and a `MachineServer`. Plain `lf -b <skill>` in a Task's checkout is bare: two context hooks, no capability, no server. A managed interactive launch, held by the server, keeps every hook; a bare one keeps two, at a terminal or held in a Desktop pane.
- Claims name their person by the repository's Git `user.email`; fixtures set a repository-local identity (`TestRepo` does).
- Fixtures that change process environment share `loopflow_test_support::env_lock`.
- macOS SMAppService fixtures register temporary bundles with LaunchServices and launch bundle-owned update code through it. An external test runner writing a registered app can fail App Management; it does not model an app update. Cleanup unregisters the unique service job and LaunchServices entry.
- `SqliteStore::open_ephemeral` applies every migration; don't replay one with `apply_migration_for_test`.
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
`lf realign` reconciles plan, code and memory.

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
- **gate:** affected suites and automated acceptance once, reusing results only for identical content and command plan.
- **CI / release:** the full matrix and deployment checks.
- **qa / demo / review:** the real product, as people use it.

Required checks run headless, with no display, permission dialog or person.
Desktop uses app builds and view tests. A check that can't run here goes to
gate or CI; it never blocks an earlier step. Real failures still get fixed.
Scratch keeps one line per check: command, result, and who runs it later.

Swift async process fixtures install `terminationHandler` before `run()` and
await its completion. Don't call blocking `waitUntilExit()` on a cooperative
executor; it can stay in Foundation's run loop after the child has exited.

Headless Swift view tests inspect production leaf views directly when a parent
needs `@Environment` Observation objects, which ViewInspector can't inject.
Keep those views internal for `@testable` access. Mounted terminal frame checks
stay in `.requiresDisplay` suites.

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
