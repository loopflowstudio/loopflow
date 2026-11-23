# LOO-921 decisions

Jack Heart, November 18: "i guess b but this sounds pretty problematic".
The Task comment requires a two-worktree mid-compile stop before registration
may stop the shared daemon. If a compile fails or the focused test turns red,
keep registration passive and describe alternatives. November 22: "go for it".
Keep the disk reserve and the cache cap. Cargo commands run serially.

Before implementation resumed November 22, the checkout had 262 GiB free.
Task status showed 142 stopped runs, no running Graph and no unfinished
Sessions listed for this checkout. The installed CLI lacked `lf ps` and
could not inspect the old run with `lf graph show <old-run-id>`.

After LOO-921 is merged and done, hand its measured baseline and cache state
to LOO-922 in a Task comment, then run `lf checkout LOO-922` and
`lf task run LOO-922 -a claude` from main with LF_* and CLAUDE* unset.

November 22 measurement choice: the original private cache and receipts
disappeared from /private/tmp. The Rust sources, lockfile and profiles at
20be2558e7 and d52cf7179e are identical. Keep both calibrated seeds and the
sccache wrapper basename in the procedure. Populate a new private cache before
measuring three pairs; exclude both warm-ups and all earlier calibration from
the comparison.

The copied, stdlib-only disk cleaner cannot import the installer (tomlkit).
Its PATH-based cache-cap observation remains separate. The 10 GiB shared cache
was full on November 16 and 22. A warm private-cache result measures the cost
of a surviving entry. Historical alternatives and calibration observations
are in d52cf7179e:scratch/loo-921-cross-worktree-cache.md.


November 22 default choice: apply the measured winner to the test profile.
Each sample ran `cargo test`; no dev-profile build was timed in this comparison.
