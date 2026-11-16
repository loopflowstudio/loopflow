# Native interaction benchmark

Extracted from LOO-906 at `b74196d33bd8e5d1ef1c7911bf2fc86a93f7ac9e`; base
`9cafc410eeab03d772b8cac7d641d6bd7edfd435`. The original branch and its remaining
rewrite stay intact. This Task supplies the first independent reviewable slice.

Jack Heart: “lets start with the perofrmance benchmark”; “so that we can use the
benchmark to prove each subsequent step”. Quiet mini preferred; current Mac may
be used when suitably idle. No OS upgrade or merge requested.

## Outcome

Use the existing native workspace/hierarchy/Session-return and committed-write
journeys as a fixed baseline for later reactivity changes. Keep measurement
sources, fixtures, build mode and host comparable across source builds. Retain
failed/interrupted/not-started observations and require twenty warm samples for
P95. Native bitmap/OCR and owned PTY replies are distinct from compositor paint,
model publication and production provider startup. No universal speed claim.

## This slice

- Extract quiet-host preflight, aggregate SQLite profile time and 10k supporting
  projection workload without importing the reactivity architecture.
- Repair the committed-write fixture to mutate current Task rows, rather than
  the old planning JSON representation. Check its SQL against a migrated store.
- Document baseline/after commands and ADEMark/Loopflow workflow coverage. Mark
  unimplemented mobile/native journeys unmeasured. No unused signpost API.
- Verify the Python comparison/report path, focused Rust instrumentation and
  Swift compilation on this checkout. Record the load rejection separately from
  harness correctness. Publish one PR; no auto-merge.

## Delete — do not maintain

Remove obsolete `pm_items.body` writes from the native committed-write fixture.
Preserve private fixture isolation, real subscription delivery, native capture,
historical snapshot preparation and timing endpoints. No schema migration.

## Checks and observations

- Source branch Python harness: 54 passed, before extraction.
- 2026-11-16 preflight: current Mac rejected, 704.9% background process CPU and
  load averages 103.83/63.67/45.57. No native trial began. Report:
  `/tmp/loo906-benchmark-host-20261115/preflight.json`.
- Mini previously observed macOS 15.5 against Desktop's macOS 26 deployment
  target; no compatible quiet-host baseline exists yet.
- Extraction: `uv run pytest python/tests/test_desktop_performance.py -q` —
  57 passed; Ruff check/format and `git diff --check` pass.
- Extracted native SQL — rename/create/cleanup and planning revision triggers
  pass against an actually migrated disposable store. The automated Python
  check exercises the native source SQL against the released normalized schema.
- `swift build --build-tests --package-path swift --jobs 2` — passed (745.85 s).
- Opt-in `WorkspacePerformanceTests` — ran and passed, 63 observations across
  256/2048/10k Tasks; debug diagnostic on the loaded host, not a native baseline.
  `/tmp/loo916-projection-diagnostic.json`, `/tmp/loo916-projection-run.log`.
- `cargo fmt --check` and `cargo clippy --all-targets --jobs 2 -- -D warnings` —
  passed. `cargo test -p loopflow --test integration
  performance_tests::cli_volume_records_real_reads_without_contents --jobs 2` —
  passed; `/tmp/loo916-rust-test.log`.
- Mini recheck: macOS 15.5, Macmini9,1, load 1.55/1.47/1.40. No OS change made.

## Remaining coverage

The coverage table is an inventory, not a claim that every ADEMark or mobile
journey is automated. Capture the first valid native baseline before using a
later rewrite slice to claim improved responsiveness. Reuse the exact measurement
contract; changing the harness requires a new baseline on both builds.
