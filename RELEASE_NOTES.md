# v0.14.13

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.13 is a maintenance release for anyone who runs `scripts/disk-cleanup.sh` on a Mac. Cleanup no longer deletes parts of caches that are still in use, so a cleanup pass can't leave SwiftPM's git mirrors half-written and break the next Mac app build. Older versions keep working against the store. This release has no compatibility break.

## Disk cleanup leaves live caches whole

The cleanup script used to delete old files and empty directories anywhere inside `~/Library/Caches`, including inside caches that tools were still using. SwiftPM's git mirrors lost their empty `refs/` directories and old packs, and the release's Mac build then failed to resolve packages. (#1633)

- Each top-level entry under `~/Library/Caches` is now judged as a whole. It is removed only if nothing inside it has been modified within `cache_idle_days`. If anything inside is recent, the entry stays.
- The summary line now counts entries rather than files: `removed N cache entries untouched for D days`.

## Operational notes

- **Less space reclaimed.** An active cache that also holds a lot of stale data is now kept in full, so a cleanup pass may free less than before. This is deliberate: a cache with one recent file counts as live.
- **Not yet exercised end to end.** The suggested check is to run `scripts/disk-cleanup.sh` on a Mac with a recently used SwiftPM cache, then build the Mac app and confirm packages resolve. #1633 notes that this walkthrough hasn't been performed.