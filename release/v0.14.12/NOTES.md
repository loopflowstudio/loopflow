# v0.14.12

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.12 is a one-fix patch. After an upgrade, a Session that was already running can keep calling an internal hook that the new `lf` no longer has. Before this release, that call made the Stop hook fail, and the provider could not end its turn. Upgrade if you run Sessions across `lf` upgrades, especially Sessions started before 0.14.11.

## Running Sessions survive an upgrade

Provider hooks call the installed `lf` entry point, so an upgrade can replace that binary while a Session is still running. A Session started before 0.14.11 still calls `__planning-check`, a hook that 0.14.11 removed. Its Stop hook then failed and kept the provider from stopping. (#1629)

- `lf` now handles an unknown command whose name starts with `__` by exiting 0 and printing a note to stderr, for example: `lf __planning-check: skipped an internal hook this lf version no longer has; restart the Session to refresh its hooks`.
- Other unknown commands still fail. A misspelled skill such as `lf no-such-skill` reports an error as before.

## Operational notes

- **Restart affected Sessions.** The skipped hook does nothing until its Session restarts. If a hook was renamed rather than removed, the renamed hook also doesn't run in that Session. The stderr note is the signal that a restart is needed.
- **Verification:** `a_hook_left_by_an_older_lf_does_not_fail_the_provider_turn` covers both the skipped hook and the failing unknown command. The fix has not yet been observed against a live Session across a real upgrade.
- No schema migrations and no compatibility breaks. Older `lf` versions keep working with the store.