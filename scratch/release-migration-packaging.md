# Source package migration order

Release candidate 42993690535 at f7faf56f91b8f1054f985612583c362167650ca5 failed its new source-package job on 2026-11-11. With LOOPFLOW_RELEASE_TAG=v0.15.3, build.rs treated all 13 slug migrations as unmerged: Cargo's extracted source lies beneath the repository's target directory, where Git has no migration history at that path. Local archive verification without release settings had missed this.

Bundle the merge order during release preparation. Cargo archives use the bundled order even when verification runs below a Git checkout; ordinary checkouts continue to use origin/main. Verify with the same release settings as CI, then rebuild and publish v0.15.3. Its existing tag and draft assets remain unchanged until the replacement candidate passes.
