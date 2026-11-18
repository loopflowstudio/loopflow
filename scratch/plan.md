# LOO-923: harness formats leave execution as a sibling of storage

## Outcome

`loopflow-harness` is a new private path crate that depends on
`loopflow-model` and nothing else in the workspace. It answers one question:
what does each native client (Claude, Codex, OpenCode, Pi) write, and where?
It holds the stream decoders and the facts they produce, the transcript
readers and the `Turn` they produce, where each client keeps its files, and
the harness identity.

`loopflow-execution` keeps everything Loopflow does with a harness: launching
it, recording it, and joining its transcripts to Sessions in the store.

```
before   model ─ storage ─ execution ─ application
after    model ┬ storage ┬ execution ─ application
               └ harness ┘
```

What changes for whoever edits Rust here:

- Fixing a decoder or a transcript reader compiles and tests one small crate.
  `cargo test -p loopflow-harness` builds neither storage, execution nor the
  application.
- A storage edit and every other execution edit stop recompiling that code,
  and execution's test binary stops carrying its tests.

No behavior, wire shape, saved data or migration byte changes.

## Demo

```sh
# a provider changed its output: fix the decoder, run its tests
$EDITOR rust/loopflow-harness/src/claude_decode.rs
cargo test -p loopflow-harness claude_decode
cargo tree -p loopflow-harness -e normal,dev | grep -cE 'loopflow-(storage|execution)'   # 0
open scripts/benchmarks/crate-rebuild/results/loo-923.html
```

The results page shows base `7bb879aff4` against the branch: a storage edit
and an ops edit both hit the harness cache, a decoder edit misses harness,
execution and the application and hits storage, and the owner's test build.

## The concepts

| A person knows | In code after this Task |
|---|---|
| A harness: Claude Code, Codex, OpenCode or Pi | `loopflow_harness::HarnessKind` and one unit struct per harness (`Claude`, `Codex`, `OpenCode`, `Pi`) |
| What that client writes: its live stream and its transcript files | `loopflow_harness::Harness`, implemented once per struct; the decoders; `transcript::{Turn, Source, read, scan}` |
| Launching and driving it | `loopflow_execution::harness::Launcher: loopflow_harness::Harness`, implemented on the same four structs |
| A Session's conversation, its answer, how long it is kept | `loopflow_execution::transcript`, which joins the store to `loopflow_harness::transcript` |

Three choices made for review:

1. **The crate has no storage dependency.** The line through `transcript/` is
   "does it open Loopflow's store". Reading a file into `Turn`s does not;
   finding which account home a Session ran in, recording a Session's turns,
   the final answer of an input and the retention summary do. Passed over:
   moving all of `transcript/` and depending on storage. That makes the crate
   one more link in the chain (storage → harness → execution), a storage edit
   (the slowest measured case, 65.5 s) recompiles it, and "what the client
   wrote" would carry `record_session`, which writes store rows. The cost of
   the choice: the crate is about 7k lines, not 15k, so its share of an
   execution compile is smaller.
2. **It is named `loopflow-harness`, not `loopflow-providers`.** `Provider`
   already names two other things in code: the credentialed service
   (`loopflow_storage::provider::Provider`: GitHub, Claude, Codex,
   OpenCodeZen, Linear, Asana, Doppler) and the rental host
   (`rental::Provider`). The code's word for the client is `HarnessKind`.
3. **One struct per harness, two traits.** `Launcher` has `Harness` as its
   supertrait and is implemented in execution on the crate's structs, so
   `Launch.harness` stays one field that answers both kinds of question.
   `kind.harness()` returns the format view; `harness::launcher(kind)` the
   launching one. Passed over: two unrelated traits on two sets of structs,
   which would give each harness two identities.

## Where each thing goes

Into `rust/loopflow-harness/src/`, with unit tests and testdata:

- `harness/{claude,codex,opencode,pi}_decode.rs`, `fact.rs`, `lf_tag.rs`,
  the recorded traces in `harness/testdata` (done, slice 1)
- `transcript/{claude,codex,opencode,pi}.rs` (the readers; OpenCode's opens
  the client's own SQLite file with `rusqlite`, not Loopflow's store)
- From `transcript/mod.rs`: `Turn`, `Receipt`, `TurnItem`, `Request`,
  `RequestSource`, `Source`, `Indexed`, `ExpiryPolicy`, `read`, `transcript`,
  `find`, `scan`, `copy_between`, `recorded_cwd`, `json_lines`, `modified`,
  the time helpers and the row folding (`Calls`, `Turns`)
- From `transcript/live.rs`: `TurnState`, `TurnUsage`, `SessionTurn`,
  `read_current`, `claude_project`, `MARKERS`
- `HarnessKind` (`parse`, `as_str`, `ALL`, `harness()`), the four unit
  structs, and trait `Harness` with: `kind`, `recognizes_driver`,
  `recognizes` (taking the command string), `title_args`, `model_rungs`,
  `sign_in_advice`, `provider_env`, `api_key_env`, `transcript_home`,
  `transcript_reader`, `transcript_id`, `transcript_root`,
  `transcript_watch_store`, `direct_transcript`, `transcript_siblings`,
  `expiry_policy`, `transcript_markers`, `human_input_times`,
  `transcript_completion_detail`, `validate_model`
- `harness::claude::usage`, which `transcript/claude.rs` calls

Stays in execution:

- `harness::Launcher` with `capabilities`, `open`, `driver`,
  `validate_agent_process_close`, `provider`, `locate_transcript`,
  `transcript_retention`, `recent_transcripts`, `conversation_owners`,
  `terminal_error`. `Capabilities` stays with it: every reader is launch,
  prompt or skill code, and it names `skills::catalog::SkillDialect`.
- `HarnessKind::from_provider` and `HarnessKind::provider` become
  `harness::from_provider` and `Launcher::provider`; they name storage's
  `Provider`.
- `transcript/{answer,facts,session,summary,watch,raw}.rs` and, in
  `transcript/mod.rs`, `adopt_native`, `conversation_home`, `locate`, `Gap`,
  `Located`, `native_home`; `live::locate`. They keep their paths under
  `loopflow_execution::transcript`, so the application's imports of
  `transcript::{answer,facts,summary,watch,live}` change only for the moved
  types.
- The four launch implementations, `fact_writer`, `native_session`,
  `native_exit`, `context`, `agent_process`, `task_updates`,
  `conformance_tests` (it drives decoders through `FactWriter` against a
  store), the launch matrix, `provider_auth`, `observe`.

The rule when a file turns out to need both: the function that takes a
`SqliteStore`, an `OsProcess` or `observe::Cache` stays in execution and calls
down. The crate never gains a dependency to keep a file whole, and execution
re-exports nothing from it.

## Upward references to remove

| # | Reference | Resolution |
|---|-----------|------------|
| 1 | `HarnessKind::harness()` returns the launch implementation (≈60 callers in execution, 4 in the application) | Format callers keep `kind.harness()`; callers that need capabilities or launch use `harness::launcher(kind)` |
| 2 | `transcript/claude.rs:117` calls `harness::claude::usage` | `usage` moves with the reader |
| 3 | `Harness::recognizes(&process::OsProcess)` | Takes the command it reads |
| 4 | `Harness::transcript_home` default goes through storage's `Provider` and `transcript::native_home` | Claude and Codex name their own home variable and directory in the crate; `native_home(provider)` in execution calls them |
| 5 | Trait methods that take the store, the process table or storage's `Provider` | On `Launcher` (list above) |
| 6 | `pub(crate)` items execution still reads (`fact`, decoders, `find`, `json_lines`) | `pub` in the crate |

## Callers and files outside Rust source

- `Cargo.toml` workspace members; execution, application and test-support manifests
- `scripts/check_architecture.py:258` and `scripts/check_migrations.py:188` crate tuples; `scripts/test.py` suite reach; `scripts/macos_inputs.py`
- `scripts/benchmarks/crate-rebuild/probe.py` member list; `run.py` cases
- `AGENTS.md` harness rule path, `rust/loopflow-execution/README.md`,
  `harness/README.md` (its "only harness dispatch" line becomes the two
  views), `docs/dev/{codebase,execution,reference-code}.md`, a new
  `rust/loopflow-harness/README.md`

## Docs written ahead of the code

They describe the crate as it will be; slices make them true and slice 3
corrects whatever the build changed.

- `rust/loopflow-harness/README.md` (new): decoder test loop, `transcript::read`
  call site, what the crate holds, the `cargo tree` check, fixture locations
- `rust/loopflow-execution/README.md`, `rust/loopflow-execution/src/harness/README.md`
  (the two views: `kind.harness()` and `harness::launcher(kind)`)
- `docs/dev/codebase.md` (territories, source owners, "Add a provider" in two
  crates), `docs/dev/reference.md`, `docs/dev/reference-code.md`
- `scripts/testing.md` (focused command, what a decoder edit rebuilds)
- `AGENTS.md` harness rule path now names `rust/loopflow-harness/**`

Layout the docs commit to: decoders at the crate root
(`src/claude_decode.rs`), stream fixtures in `src/testdata/`, transcript
fixtures in `src/transcript/testdata/`, `transcript::read(HarnessKind, &Source)`
keeping today's signature. `docs/dev/execution.md` needed no edit.

## What must keep working

- Native provider conversations and hooks: the launch matrix and
  `conformance_tests` pass unchanged in execution.
- One application launch or resume test and one moved fixture run after the
  move; compilation alone misses runtime script paths.
- Served and remote commands, process identity, Desktop and iOS inputs
  (`swift/ios.yml` reads `rust/loopflow-execution/tests/fixtures/dto/server`,
  which does not move). `Turn` and its parts keep their serde shape.
- A second build with no edit stays fresh.
- `loopflow-storage` and every migration file are untouched.

## Acceptance check at gate

```sh
cargo test -p loopflow-harness
cargo tree -p loopflow-harness -e normal,dev | grep -cE 'loopflow-(storage|execution)'    # 0
uv run python scripts/test.py --reuse-passing --rust-filter 'test(harness::)'
uv run python scripts/benchmarks/crate-rebuild/run.py --base 7bb879aff4 --candidate HEAD --sccache <path> --output /tmp/lf-923
```

`run.py` gains two cases. `harness_format` edits a decoder message at each
revision's own path and requires harness, execution and application misses
with a storage hit. `harness_test` builds the owner's test binary after that
edit (`--no-run`) and records its compiler closure and link. The existing
`execution` and `storage` cases additionally require a harness hit on the
candidate. The report keeps wall and CPU seconds, peak RSS and closures, as
LOO-650's did.

## What the evidence says, and what it does not

How to get each number again is in parentheses.

- **Execution is one cycle.** 50 of 64 top-level modules (112k of ~115k
  production lines) form a single strongly connected component by `crate::`
  references. No sibling exists today; every extraction starts by removing
  upward references. (Module graph over `rust/loopflow-execution/src`, test
  modules cut at `#[cfg(test)] mod tests`.)
- **PRs are whole-feature journeys.** 228 PRs with production Rust churn, 215
  in today's execution code. A PR that touches any execution group also
  touches the application 77–97% of the time and storage 58–96%. No two
  execution groups co-change above 71% except with `ops`.
  (`cochange.py --count 300` at `7bb879aff4`, minus the three split PRs and
  rename PRs #1969 and #1643.)
- **Inside the harness there are two things.** Format files change in 23 of
  215 execution PRs, launch files in 51; 17 of the 23 also touch launch, 17 of
  the 51 touch format. One trait carries both today.
- **The format files are already leaves.** The decoders, `fact`, `lf_tag` and
  the four transcript readers import `loopflow_model` and `rusqlite` only.
- **Timings to beat** (LOO-650, `results/loo-650.json`, one Cargo job,
  `incremental = false`): application edit 27.4 s, execution edit 57.5 s,
  storage edit 65.5 s; peak RSS 3.4 GiB when execution compiles.

What this extraction can and cannot buy, by the same numbers:

- A crate below execution saves every other edit only that crate's compile
  share. For about 7k of 115k lines that is a second or two of 57.5 s.
- The "one small crate" loop reaches a PR only while it stays inside format
  files: 6 of 215 PRs. The other 17 format PRs also rebuild execution.
- The edit that pays most is the one made most. `ops` is in 110 of 184
  grouped execution PRs, 34 alone; as a crate above the rest it would spare an
  ops edit about 20 s of 57.5 s. It is not this Task's first extraction
  because the Task prefers siblings to a longer chain and 17 lower modules
  reference `ops` (42 references; `engine` and `machine` another 78). It is
  question 1 on the review page.

So this Task's first PR is the representative extraction the Task asks for,
and its measurement decides what follows. If the ops-edit and storage-edit
drops are inside run-to-run noise (LOO-650's range was under 1 s), siblings
this size are not worth their boundary, and the next Task is chosen by
measured compile share: `ops` above the rest, not another leaf.

## Sequencing and overlap

- LOO-921 (cache and test baseline) is idle on an open question and LOO-922
  (fresh-worktree cost) is unstarted. The extraction does not depend on
  either; the comparison against their results does. Choice made for review:
  build the slices now, measure base against candidate with `run.py`'s private
  caches, and state in the PR which predecessor numbers did not exist. Before
  `run.py` runs, `lf task status LOO-921` and `LOO-922` show neither is
  measuring: one expensive measurement at a time.
- Open PRs editing files that move or split: #1967 (eight `transcript/`
  files, `claude_decode`), #1976 and #1972 (`transcript/raw.rs`,
  `transcript/mod.rs`, `harness/mod.rs`), #1968 (`harness/native_session.rs`).
  Whichever merges first, the other rebases across a rename; this branch takes
  their content and changes none of their behavior.

## Slices

1. **Spike: the leaves. Built (`5848e85bea`).** The four decoders, `fact`,
   `lf_tag`, their tests and the seven recorded traces are in
   `loopflow-harness`; execution depends on it and the old files are gone.
   What a run showed, 2026-11-18, this Machine, default Cargo jobs, sccache on:
   - `cargo test -p loopflow-harness`: 62 passed. `cargo tree` check: 0.
   - Owner's test build after a one-string decoder edit
     (`cargo test -p loopflow-harness --no-run`): 0.63 s, and 0.88 s with
     `RUSTC_WRAPPER=`. The same loop through execution's test binary after an
     execution edit: 34.1 s. Neither is `run.py`'s one-job number; slice 3
     measures those.
   - Execution's `harness::conformance_tests`, `fact_writer` and `pi_tests`:
     40 passed against the crate.
   What it taught:
   - Surface that became `pub`: the three decoder structs with their methods,
     `ReaderState`'s four readers, `codex_decode::{turn_request, rpc_error}`,
     and all of `fact`. Nine Codex mapping functions and `ItemPhase` became
     private instead: nothing outside the file read them.
   - Mixed fixtures need one thing: `fact::trace` behind the crate's
     `test-support` feature, which execution's dev-dependency and its own
     `test-support` feature enable. No shim, no second copy of the traces.
   - Two upward references the table missed: `opencode_decode` called
     `opencode::sanitize_error_message` (moved down with its tests; needs
     `regex`), and the decoders use `uuid` and `tracing`. The crate's
     dependencies are model, `anyhow`, `regex`, `serde_json`, `tracing`, `uuid`.
   - `pi_rpc.py` did not move: it is a stand-in Pi process for launch fixtures
     (`pi_tests`, the launch matrix, two application test binaries), not a
     recorded stream. It stays in execution's `src/harness/testdata/`.
   - `scripts/check_architecture.py` and `scripts/check_migrations.py` now
     scan the crate; both pass. The third-party inventory was refreshed for
     the `Cargo.lock` change.
2. **Split the trait, move the readers.** `HarnessKind`, the unit structs and
   `Harness` move; execution gets `Launcher: Harness`; the table's rows are
   resolved; the transcript readers and the store-free half of
   `transcript/mod.rs` and `live.rs` move. Run the launch matrix and one
   application resume test.
3. **Measure and document.** `run.py` cases, `results/loo-923.html`, scripts,
   READMEs and `AGENTS.md` paths.

## After this Task

Each its own linked Task, ordered by this Task's measurement and not by this
list:

- `ops` as the crate above the rest of execution, if question 1 is answered
  yes or the sibling drop is inside noise.
- Trackers: `linear`, `asana`, `ops/linear`. 86% of execution PRs skip them;
  about 11 upward references outside `ops/linear`.
- Definition resolution into `loopflow-definitions`: `config`, `graph`,
  `skills`, `definition`, `prompt`, `task_graph`. 71% skip; about 70 upward
  references, `config → store` first.
- Git and repository facts. 89% skip; blocked on `observe`, which reads
  transcripts.
- Harness logins and the launch half. They need the store, so they join
  `loopflow-harness` only if that crate is to depend on storage; otherwise
  they are a crate of their own between storage and execution.
