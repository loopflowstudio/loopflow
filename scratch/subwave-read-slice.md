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

The checkout integration test exercises direct Wave binding through an isolated
registry and checks the goal and memory each appear once. The nested collector
also explicitly selects its already-included goal. Final verification passed on
2026-09-30 after disk capacity recovered.

## Earlier resource blocker

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

## Completed verification · 2026-09-30

Resource recovery passed with 44.1 GiB free against the 32 GiB reserve. The uv
cache remained busy; recovery retained active and recent builds. The later disk
sample was 42 GiB free. No forced cleanup was used.

Removed the redundant diff-file deduplication and the duplicate early-return
path in `gather_documents`; its single final deduplication preserves first-source
ordering and the explicit-doc limit.

All seven recorded resume commands passed with inherited LF_/LOOPFLOW_ authority
removed and a disposable Home: 16 context tests, 74 prompt tests, two goal-render
tests, eight native-seed tests, one attributed-context test, one direct-Wave-binding
test and one golden test. Log: `.lf/tmp/subwaves/final-read-tests.log`.
These resolve the previously unexecuted read-slice regressions. Parent/schema
verification belongs to the [working design](define-durable-subwave-identity-and.md).
No full gate, configured provider demo or installed-Home operation ran here.
