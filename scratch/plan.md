# LOO-988: four flaky tests pass every run

Four tests named in the brief. Two were already fixed on main; one has a
reproduced root cause; one has a cause found by reading. One PR.

## Findings (2026-11-19, base 2750c9e0fb)

### 1a. `resuming_an_ended_conversation_launches_its_provider_as_the_session`: fixed on main

Cause at a0548bfe28: `resume_refuses_a_run_and_leaves_its_provider_running`
called `journal::with_process`, which sets the process-wide `PROCESS_STARTED_AT`
OnceLock (`rust/loopflow-execution/src/journal/mod.rs:301`). After that,
`is_cli_process()` is true for the whole test process. `with_runtime` skips
admission (`journal/mod.rs:352`), `current_context()` reads the empty
`PROCESS_CONTEXT`, and `native_session::admit` fails with "Native launch has no
admitted invocation". Which test failed depended on order. #1972 (f5ff6a6c79)
replaced that call with `with_runtime`. No other code outside `bin/lf.rs:1509`
calls `with_process`.

Run at HEAD: `cargo test -p loopflow-execution --lib human_session` passed 51/51
in 7 plain runs and 12 `--shuffle-seed` runs (1–12).

What's left is making the trap impossible to write: `with_process` becomes
`#[cfg(not(test))]`, so a lib test that calls it no longer compiles. The binary
is its only caller.

### 1b. `task_conversation_preparation_is_passive_and_connect_uses_shared_launch_settings`: reproduced, cause found

Fails every time on the first run after `lf` is linked (3/3), and passes on
reruns (7.8s):

```
Error: Holder at /private/tmp/lf-501-…/holder-03ff1bbd8afe.sock is not answering: No such file or directory (os error 2)
```

The private directory held `holder-aef4a0f1a41a.sock`, so the holder bound a
different build's place. What happens:

1. `cargo test` builds `lf` with dev-dependency features unified
   (`deps/lf-63765936…`, sha 03ff…) and hard-links it to `target/debug/lf` =
   `CARGO_BIN_EXE_lf`.
2. `Fixture::new` starts `MachineServer` from `target/debug/lf` (03ff). Its
   registry keys the holder `Place` by that build.
3. The test then calls `support::EnvGuard::with_lf_home`, which calls
   `loopflow_test_support::lf_binary()`
   (`rust/loopflow-test-support/src/lib.rs:154`). That runs
   `cargo build --offline -p loopflow --bin lf`: no dev features, so a different
   unit (`deps/lf-63f37cbb…`, sha aef4…), linked over `target/debug/lf`. A watcher
   caught that cargo process mid-test and saw the file change.
4. The server spawns `current_exe()` for the holder, and that path now holds
   aef4. The holder binds `holder-aef4….sock`. The server polls
   `holder-03ff….sock` until the 20s `STARTUP` deadline
   (`terminal_holder/client.rs:18`).

Load only stretches the timing. The swap is the cause. CI never sees this,
because it sets `LOOPFLOW_TEST_LF` (`.github/workflows/ci.yml:256`, as does
`scripts/test.py:282`), and that skips the nested build. Running with
`LOOPFLOW_TEST_LF=$PWD/target/debug/lf` right after a fresh link passed in 6.5s,
with `target/debug/lf` still 03ff afterwards.

Decision: a loopflow integration test binary hands `lf_binary()` the CLI it was
built with. The ctor in `rust/loopflow/tests/support/mod.rs` sets
`LOOPFLOW_TEST_LF` from `option_env!("CARGO_BIN_EXE_lf")` when it is unset.
This is the same mechanism CI uses. Test processes no longer run a nested build,
and no `lf` gets linked over a running one. Lib unit tests have no
`CARGO_BIN_EXE_lf`; they keep the nested build. Their own processes all run that
one build, so nothing swaps under them.

What the ctor does not cover: the fixture CLI is a path, `target/debug/lf`,
and two builds of `lf` link to it. The ctor removes the writer inside an
integration binary. Lib test binaries (`cargo test -p loopflow --lib`, which
holds `tests/work/`, and loopflow-execution's tests) have no
`CARGO_BIN_EXE_lf` and still run the nested build, so a bare `cargo nextest run`
without `LOOPFLOW_TEST_LF` can still relink the path under an integration binary
running beside them. `scripts/test.py` and CI set the variable, and plain
`cargo test` runs the lib binary before the integration binaries. Listed under
Exclusions; the structural answer is on the concept-review page.

Rejected options:
- Building the same variant in `lf_binary`: it would have to replicate Cargo's
  unified feature set and profile, which is fragile.
- A separate target dir: a second full build of `lf`.
- Dropping the holder deadline and waiting on the lock: with a swapped binary,
  the wait would hang forever instead of failing.

The holder side does get a named failure. The ready line the holder prints
(`terminal_holder.rs:108`) gains its build: `<pid> <build>`. `spawn` returns it,
and `Client::ensure` fails at once when it differs from `place`, instead of
polling 20s at a socket nobody will bind. The error says what happened, not just
what differs: the executable at `<path>` is now build `<b>`, this server is
build `<a>` and has been replaced since it started. Production can reach this
too: `lf install` commits the entry gate with `fs::rename`
(`installation.rs:753`), and a server still running spawns `current_exe()`. The ready line is read only by `spawn`, so it stays one line. A line with
no build (an older holder) is treated as a match.

### 2. Swift "New Task closes the sheet in the click…": cause found by reading

These steps are from reading the code, not from a run:
`openCreatedTask` (`swift/LoopflowMac/WorkModel.swift:2665`) →
`openTaskDestination` → `select(.task)` → `openSelectedWorkspace` → `openWarmConversations`. Then
`prefetchConversations` (`WorkModel.swift:330`) adds the repository primary, and
`SessionsWorkspaceRegistry.openWarmConversations`
(`swift/LoopflowMac/Views/SessionsView.swift:604`) starts unstructured
`Task`s. Each calls `ensurePrimarySession`, which issues `session resume …`
(`RegistryQuery.swift:416`). Nothing awaits them, so whether they log before the
test reads `requests` depends on executor order.

Decision: the test sets `registry.canPrepare = { false }`, the existing guard at
`SessionsView.swift:608` (also used by `WorkCacheTests.swift:451`). Warm openings
of other workspaces are not what this test covers. The assertion keeps its
meaning: the created Task issues nothing after its answer. Primaries opening on
load is intended (`SessionsView.swift:535`), so production stays as it is. The
sibling test "The answer's row selects a Task planning never lists, with no other
read" (`WorkDestinationTests.swift:249`) gets the same line if it goes through
`openCreatedTask` with the same assertion.

The first step confirms the cause before the fix lands: in the test, await the
registry's openings, then assert the log contains `session resume`. If it
doesn't, the cause is something else and this section gets rewritten.

### 3. Codex service-tier test: fixed on main

#1982 (3496a8761d) split `codex_service_tier_reaches_native_and_headless_launch_and_resume`
into six tests (`rust/loopflow/tests/context_launch_tests.rs:110-136`). Measured
serially at load ~100: 5.0–9.9s each. Nothing to do. The neighbour
`codex_startup_failures_preserve_input_and_allow_reconnect` took 44.9s here and
74s/94s in CI against the PR's 120s; it's noted in `questions.md` and left out of
scope.

## Done when

- `touch rust/loopflow/src/bin/lf.rs && cargo test -p loopflow --test session_launch_contract`
  passes on that first run, and `target/debug/lf` hashes the same before and
  after.
- `cargo test -p loopflow-execution --lib human_session` passes, including under
  shuffled seeds; a lib test that calls `journal::with_process` does not compile.
- WorkDestinationTests' New Task test cannot see a warm-primary request; CI's
  Swift view tests pass.
- A holder that answers for a different build fails at once, naming both builds.

## Acceptance check at gate

```sh
touch rust/loopflow/src/bin/lf.rs
cargo test -p loopflow --test session_launch_contract --no-run
shasum -a 256 target/debug/lf    # A
env -u LOOPFLOW_TEST_LF cargo test -p loopflow --test session_launch_contract
shasum -a 256 target/debug/lf    # still A; 27/27 pass
```

## Slices

1. **No nested build under loopflow integration tests.** The ctor sets
   `LOOPFLOW_TEST_LF` from `CARGO_BIN_EXE_lf`. Run the acceptance check above.
   This one can prove the plan wrong: if a nested cargo still runs (watch
   `pgrep -f 'cargo build --offline -p loopflow'`), some binary misses the ctor.
   Check every standalone `[[test]]` in `rust/loopflow/Cargo.toml` that calls
   `lf_binary()` (e.g. `build_cleanup.rs`) includes `support`, or points it at
   `CARGO_BIN_EXE_lf`. Update the `LOOPFLOW_TEST_LF` paragraph in
   `rust/loopflow/tests/README.md:304`.
2. **Holder ready line names its build.** `run()` prints `<pid> <build>`;
   `spawn` returns it; `ensure` fails at once on a mismatch. Unit test in
   `terminal_holder/tests.rs`: a ready line naming another build makes `ensure`
   return the named error with no 20s wait.
3. **Swift:** confirm the warm-primary request, then `canPrepare = { false }`.
4. **`with_process` is `#[cfg(not(test))]`.**

## Exclusions

- `codex_startup_failures_preserve_input_and_allow_reconnect` speed.
- Concurrent cargo commands in one worktree clobbering `target/debug/lf`
  (AGENTS.md: one cargo command at a time).
- Lib test binaries' nested builds relinking `target/debug/lf` beside a running
  integration binary under bare `cargo nextest run` (see 1b).
- Spawning the holder from a build-owned executable instead of `current_exe()`
  (concept-review page, question 1).
- Waiting on the holder lock instead of the `STARTUP` deadline when a
  same-build holder is claimed but refusing.

## Concept review (2026-11-19)

`scratch/concept-review.html`. The four fixes stand. Added: what slice 1 leaves
uncovered, and slice 2's error names the replaced server. Chosen for review:
slice 2 stays a named failure; making the holder executable belong to its build
(`Place` carries the executable the server spawns) is passed over here as
larger than a flaky-test Task.

## Docs (write-docs, 2026-11-19)

Changed: `rust/loopflow/tests/README.md` (the `LOOPFLOW_TEST_LF` paragraph:
integration binaries set it from their own `lf`; lib binaries and bare nextest
still need it set). Slices 2–4 change no user-facing docs; slice 2's error text
is its own documentation.
