# Subwave read slice · LOO-354 · 2026-09-30

## Finish line

A nested Wave prompt includes each ancestor's top-level Markdown, then its
own, exactly once, from the executing checkout. Ordinary runs exclude children,
siblings and unrelated Waves. Scratch stays recursive. Assembled prompts and
native skill seeds share collected documents. No separate memory registry read
or preassembled memory input remains.

## Changes and review

Ported only the LOO-329 read slice from `ce97003cf` through
`backup/define-durable-subwave-identity-and-20260930`, adapting the renamed
Exec preparation and native seed APIs. No migration file changed.

Concrete review findings fixed after the initial test pass:

- Direct Wave selection forced its canonical checkout. Preserve the invoking
  checkout when it belongs to the Wave's repository; explicit selection from
  another repository still places at the Wave's repository.
- The Wave seed copied GOAL.md's body before ordinary context included the whole
  file. The seed now gives operating direction and available Flows; gathering
  alone supplies authored Wave files.
- Explicit docs and changed-file selections could repeat a Wave file. Deduplicate
  the gathered documents by path while preserving the first source and ordering.

The checkout integration test now exercises direct Wave binding through an
isolated registry and checks the goal and memory each appear once. The nested
collector test also explicitly selects its already-included goal. Both final
regressions are **authored, not executed** after the resource stop. The golden's
Wave fragment was ported from LOO-329; final golden verification is still owed.

## Evidence and blocker

- Initial resource preflight passed with 35.9 GiB free above the 32 GiB reserve.
- `cargo test -p loopflow --test context_tests --lib wave_` passed: 38 unit tests
  and eight context tests, with inherited `LF_*` / `LOOPFLOW_*` variables removed.
  Log: `.lf/tmp/subwaves/read-tests.log`. This precedes the three review repairs
  above, so it is not a final-tree pass.
- `uv run python scripts/resource_envelope.py --recover` then failed at
  **31.7 GiB free / 32 GiB reserve**. It retained active and recent builds and
  found no eligible build cleanup. Its uv cleanup failed on the busy cache lock.
- Direct `UV_LOCK_TIMEOUT=0 uv cache prune` also failed on that lock. No forced
  cache cleanup or removal of another active checkout's artifacts was attempted.
- Final `cargo fmt --check` and `git diff --check` passed. No commit was made
  because the required all-target Clippy check could not run.
- The final disk sample still reported only 31 GiB free.
- TESTING.md prohibits starting product verification below the emergency reserve.
  Stop dependent parent/schema work until final reader proof can run. No branch
  binary accessed the installed Home. No live PM, cron, PR or Task mutation ran.

## Resume

Compression inspected the full read-slice diff and its gathering, rendering,
native-seed and checkout-resolution callers on 2026-09-30. No further code
edits were made: recovery again failed, now at **30.0 GiB free / 32 GiB reserve**,
and a subsequent direct `UV_LOCK_TIMEOUT=0 uv cache prune` failed on the busy
cache lock. Active and recent builds were retained. No product tests or Clippy
ran, and no checkpoint was made.

One reduction remains worth applying once verification can run:
`gather_documents` now deduplicates all sources, making the later
`gather_context` diff-file deduplication redundant. Put the optional file gather
inside `if spec.include_files` so one final deduplication serves both paths.
Preserve first-source ordering and the explicit-doc file limit.

After the resource envelope permits verification, run with inherited `LF_*` and
`LOOPFLOW_*` variables removed and disposable Homes for CLI executions:

```bash
cargo test -p loopflow --test context_tests
cargo test -p loopflow --lib engine::prompt::tests
cargo test -p loopflow --lib engine::flow::tests::render_goal
cargo test -p loopflow --lib lf::commands::run::tests::skill_exec_seed
cargo test -p loopflow --lib lf::commands::run::tests::attributed_context
cargo test -p loopflow --lib ops::run::tests::direct_wave_binding
cargo test -p loopflow --test golden_prompt
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Then continue parent discovery, the release split and the complete Task-bound
prompt/rename proof in the restored design. This pass does not satisfy those
requirements or the interactive demo. No full gate was run in implement.
