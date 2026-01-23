# v0.6.11

This release adds watch and cron trigger support for background agents with automatic retry and circuit breaker protection. It also simplifies context configuration by consolidating goals into voices and replacing multiple diff flags with a single `--diff-mode` option.

## Changes

- Add watch mode for file-change triggered agents with glob pattern support
- Add cron mode improvements for scheduled agent triggers
- Add retry with exponential backoff (3 retries, 30s) and circuit breaker (trips after 5 failures)
- Consolidate goals into voices with global voice support via `~/.lf/voices/`
- Replace `--diff/--diff-files` flags with single `--diff-mode` option (FILES, DIFF, NONE)
- Add explicit agent mode field (`loop`, `watch`, `cron`) to data model
- Clean up temporary agents after `lfd run` completes
