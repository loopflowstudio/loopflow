# LOO-629: scheduled release-run keeps failing

## Cause (from cron receipts, obligation records and release evidence)

- 2026-10-21: scheduled run selected v0.14.13 (`aa7038f`, workflow 39652090185) and failed
  in `publish_release.py publish` (`docker info` timed out after 10s).
- v0.14.13 was then published by hand, followed by v0.14.14–v0.14.16. The 10-21 opportunity
  never settled, so `accounting::begin` carried its selection into every later attempt
  (`~/.lf/cron/obligations/73c3…json`: receipts cron_b966… on 10-23 and cron_be61… on 10-26).
- Each resume ran `finish_candidate` → `verify_source` at `aa7038f`. `check_migrations.py`
  compares that tree against the live `origin/main`, which gained `0.14.15.001_release.sql`
  on 10-22, so it fails with "exists on origin/main but is missing from this branch". The
  10-23 run had failed earlier on a CLI argument error (`--flow`).
- Even past verification, the resume couldn't finish: `reconcile --tag v0.14.13` refuses
  because the latest release is newer.

## Fix

`release_run_with_cron`: when a saved selection's tag is published and a newer tag exists
(`superseding_tag`), record `superseded-release-selection` evidence as the attempt's candidate
replacement and run the ordinary release from current main.

## Check

`cargo test -p loopflow --test integration scheduled_release_tests` — 13 passed (new
`superseded-candidate` fails without the fix with "release verification", the production
error); clippy/fmt clean. Live proof waits for an installed lf with this fix (questions.md).
