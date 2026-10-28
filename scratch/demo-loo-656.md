# LOO-656 demo (headless, 2026-10-28)

Reviewed: the top-3 pursue (refusals keep the store and name the build to
install; Swift/MIGRATIONS docs; reader catalog + `can_open`) through the branch
binary `lf 0.15.0+b884d9c83`. No person present; the agent stood in for Jack
at the demo step. Design: `scratch/loo-656-install-lf-from-main.md`.

## Observed

- `lf self install reader-catalog` with empty `HOME`/`LF_HOME` prints the
  released batches through `0.15.0.001_release` and creates no home.
- Fresh private `LF_HOME`: opens, migrates, `lf session list` → "No Sessions."
- Private `LF_HOME` whose ledger holds an unknown breaking row
  (`future_breaking_slug`, floor itself): refuses with "database migration
  future_breaking_slug is unknown to lf 0.15.0+b884d9c83 (latest known
  0.15.0.001_release); a newer lf migrated this store; install it with
  `lf self install --main`". File kept.
- Your real `~/.lf` (installed lf is 0.14.19, ledger head
  `0.14.19.002_release`): the branch build validates read-only and refuses with
  "shared store … does not have migration 0.14.20.001_release yet; install a
  build that carries it with `lf self install --main`". Nothing written. The
  machine home resolves from the passwd entry, so a temp `HOME` doesn't isolate
  it; only `LF_HOME` does.
- Drifted schema message ("last migrated by lf 0.14.2 (012345678)"): proven by
  `a_drifted_store_names_the_build_that_last_migrated_it` and
  `a_skipped_migration_keeps_the_store_…`; `can_open` by
  `a_candidate_opens_a_store_only_when_its_catalog_knows_the_store_floor`. All pass.
- No refusal suggests deleting `loopflow.db`.

## Interpretation

- A private `LF_HOME` with nothing pending skips schema validation, so a
  hand-dropped index there opens without complaint. That predates this change
  (private homes are disposable). Not a gap in the pursue.
- `package-build.yml` ships `reader-catalog.txt` per artifact; only a green
  `Packages (main)` run after merge proves the download path.

## Next

Advance to the PR step. Proof still owed after merge: first real
`lf self install --main` from a green `Packages (main)` run moves your
`~/.lf` from 0.14.19 forward (backup first) and the branch build stops refusing.

Check: branch `lf` real-path runs above + 3 focused `store::migrations` tests passed at b884d9c83.
