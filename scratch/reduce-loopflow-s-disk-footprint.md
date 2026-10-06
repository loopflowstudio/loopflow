# Reduce Loopflow's disk footprint (LOO-390)

Status: implemented, awaiting gate. Jack Heart authorized autonomous design,
cleanup, implementation and delivery on 2026-10-06 (steer `ea6171d3`).

Evidence, inventory, 5 Whys, growth model and producer audit are durable in
`docs/reviews/storage-footprint.md`; this note holds only what the delivery
still needs.

## Decisions (2026-10-06, made under Jack's delegation)

- SQLite history keeps what streaming increments add up to; `events.jsonl`
  stays verbatim. Chosen over dropping raw provider lines from SQLite, which
  would save more but changes which store is complete. That choice is left open
  in the review as proposal 1.
- Install preflight reads an exact-frontier store in place. A pending migration
  still validates against a migrated private snapshot, copied by PR #1465's
  pinned backup (merged, v0.13.7, already in this branch). #1465 made that copy
  finish under writers; it did not remove the per-probe copy this change removes.
- Write connections set `journal_size_limit` to 64 MiB.
- A committed migration keeps the two newest fingerprinted backups. Hand-named
  and unfingerprinted files are never matched.
- Existing rows, legacy directories, binaries and other Sessions' scratch were
  not touched. No schema migration was needed.

## Delete — do not maintain

Nothing is slated for removal. Behavior to preserve: streamed-prose recovery
after a capture directory is deleted
(`final_answer_reader_recovers_legacy_streamed_prose_honestly`), snapshot
validation for pending migrations, and backup reuse by history fingerprint.

## Cleanup performed

27.9 GiB: eleven stranded `candidate.db` copies in `$TMPDIR` (10.4 GiB) and
`cargo clean` in `loopflow.make-the-release-and-ci` and `loopflow.resume`
(17.5 GiB). Before/after `df` is in the review.

## Remaining

- Gate: affected Rust suites. The two `session_record` tests that open a Home
  fail under an inherited `LF_HOME`; run with `LF_*` cleared.
- After a release installs this: observe the WAL shrink, no new `candidate.db`
  after a currency probe, backups pruned at the next migrating release, and the
  per-capture row count. None is observed yet.
- Proposals 1–9 in the review need sponsorship; retention (2) and the system of
  record (1) are decisions about what history is worth, not implementation gaps.

- Acceptance not met by this PR: growth is reduced, not bounded. No capture,
  binary or Codex-home retention exists; `lf home doctor` reports no sizes; the
  interrupt-cleanup gaps are documented, not fixed. The Task stays open on these
  unless Jack accepts the review's proposals as the remaining scope.

Reconciled 2026-10-06 against `41b86d141` (no newer main): code, review, data
doc and memory agree; the review's probe path and `MIGRATIONS.md` backup
retention were corrected.

Check: all `LF_*` unset, `cargo test -p loopflow --lib -- session_record` — 49 passed after compress; clippy `-D warnings` clean. Realign changed prose only, no rerun. Other suites unchanged since the 149-pass run; gate owns them.
