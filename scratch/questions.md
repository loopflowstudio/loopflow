# Open questions and assumptions — ENG-23

Headless run: decisions made with best judgment, recorded here for the reviewer.

## Assumptions taken

- **`DRAFTS` lives in `migrations.rs` (or a sibling `drafts.rs`), not a separate crate
  or a runtime-scanned directory.** `include_str!` keeps drafts compiled-in and
  toolchain-checked, matching how `MIGRATIONS` works. A runtime directory scan would
  lose the "unregistered file never runs" guarantee the current check depends on.

- **The upgrade fixture is a committed binary SQLite file** (`previous_release.db`),
  regenerated in the release PR. Considered a pure SQL-seed alternative that
  reconstructs the prior frontier by slicing `MIGRATIONS`; rejected because a binary
  fixture at the real prior frontier is the most faithful reading of "a fixture at the
  previous released frontier" and exercises real prior data. If the reviewer prefers no
  binary in git, the SQL-seed-plus-slice variant is a drop-in substitute (note the
  slice boundary must be recorded by the release step, not hardcoded).

- **Recreate-on-draft-drift deletes and re-initializes the whole dev store** (after a
  backup), rather than resetting only to the canonical frontier. Resetting product data
  built by a draft is not possible without dropping tables, and the dev store is
  explicitly disposable, so wholesale recreation is the honest response.

- **028/029 is left to the operator.** The design does not renumber released ordinals;
  the numeric-tuple ordering tolerates a one-time gap if #1052 lands as `0.11.028`.

## Genuinely open (reviewer's call)

1. **Draft-to-canonical body transform.** Does canonicalization copy the draft SQL body
   verbatim (stripping only the `-- name:` / `-- depends_on:` header), or should the
   canonical file retain a provenance comment (`-- canonicalized from draft add_x at
   v0.11.30`)? Verbatim keeps checksums a pure function of the SQL; a provenance comment
   aids humans but changes the checksummed bytes. Leaning verbatim.

2. **Where the upgrade gate runs in `lf release run`.** It must pass before the tag.
   The release PR's CI already runs `cargo test` (which includes the gate), and
   `release_run` waits for merge — so CI is the gate. Should `release_run` *also* invoke
   the gate locally for fast pre-PR failure, or rely solely on PR CI? Leaning: rely on CI,
   optionally add a local `lf release check`-time invocation later if the round-trip hurts.

3. **Seed maintenance ownership.** The upgrade seed must stay valid at the previous
   frontier as schema evolves. Is updating the seed part of authoring a data migration
   (draft author's job), or a release-time chore? Leaning: the draft author extends the
   seed when their migration touches seeded tables, verified by the gate test failing.
