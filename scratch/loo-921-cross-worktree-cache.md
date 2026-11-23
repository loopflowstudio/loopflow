# LOO-921 — cross-worktree Rust caching and the edit-to-test loop

## Design and decisions

Install one pinned, local-disk sccache in personal Cargo configuration. Use
client-side compilation, one socket per pin, a shared cache directory, and
one target per worktree. Preserve unrelated personal Cargo settings and Git
hook content. No wrapper or cache settings in CI. The wrapper path carries
the revision because stock and pinned binaries both report version 0.19.0.

The Machine has one root inventory. The daemon reads it at startup. In 1h45m
on November 16, the daemon had no
300-second gap between cache-file accesses (longest 249s) and 4 of 64 roots
were still unloaded. Stats RPCs also reset its idle timer. A shorter timeout
cannot take effect until this daemon exits. The November 18 direction selects
registration-triggered stop conditional on an in-flight compile surviving it.
Use a private socket shared by two linked worktrees for this decision.

Measure one focused test passing after each semantic edit, including test
compilation and linking. Use the installed pin, personal jobs setting,
separate private cache and alternating cache/incremental groups. Seed app and
execution in each mode. For each mode: fresh app edit, two further app edits,
execution edit in the same target, two further execution edits. Also measure
a fresh execution-only worktree in cache mode. Record at least three complete
paired cohorts. Select incremental for the test profile only if it wins the median
three-edit total in both app and execution. Concurrent-worktree capacity is
unmeasured; the November 16 delivery comment says to "report concurrent-worktree
capacity as unmeasured unless a separate permitted experiment exists".

## Checks and remaining work

- November 16: private root and real-crate probes passed, including post-idle
  reuse, semantic edits, checkout identity and source-path observations. Four
  corrected seed commands passed. The original cohort exposed a cold
  incremental graph and a receipt wrapper basename that bypassed cc-rs.
  No measured warm rows from that run. Full observations and check commands:
  d52cf7179e:scratch/loo-921-cross-worktree-cache.md.
- November 22: the first mid-compile stop left rustls PID 42534 running and
  its focused execution test passed in 7m20s. The installed-hook probe then
  registered a second worktree while rustc PID 41631 compiled loopflow_execution.
  Registration and old-daemon exit took 11.016s. The same compiler outlived it,
  the replacement loaded the root, and the focused application test passed.
  Cargo produced the execution rlib, which was absent from the private cache;
  the model control rlib was present. The receipt is in results/loo-921.json.
- Three warm pairs with two Cargo jobs passed all 39 exact tests. Median
  three-edit totals (cache / incremental): application 284.52 / 228.03s,
  execution after application 259.75 / 131.45s. Enable incremental for the
  test profile; keep dev builds cacheable. Results and ranges are in
  scripts/benchmarks/crate-rebuild/results/loo-921.html.
- Installed Machine hook: the fresh-root execution test passed with the selected
  incremental profile in 259.79s from Git checkout (one job). With a unique
  edit in two more roots, only loopflow-definitions rebuilt. The producer
  recorded 0 hits / 1 miss; the consumer 1 hit / 0 misses and a byte-identical
  rlib. Checking for an absolute path in `strings` did not identify the producer.
  The report retains the per-invocation counters and hashes. The opening
  Machine status was 100/100 loaded, 10 GiB/10 GiB occupied.
- `uv run pytest -q python/tests/{test_build_cache,test_dev_lf}.py` passed all
  17 cases (rerun November 22 after renaming the writer). They cover skipped-hook
  recovery, the stale daemon, missing inventory with a loaded root, caller-owned
  configuration and the isolated build. A linked checkout with its hook skipped returned 3 without creating
  the inventory. Manual registration listed both roots and status returned 0.
- With the installed pin on a private socket, a stale daemon survived both
  status and caller-owned registration. Machine registration stopped it. The
  next plain Cargo build passed with the second root loaded by a different PID.
- Gate owns `documented_commands::` and the broader suites.
- The Machine's installed hook copy was still at the pre-recovery version on
  November 22. Reinstall from the merged checkout during verification to update
  the copy and check the live checkout path.

## Registration and recovery

Installation seeds the inventory. Git's fail-soft hook and manual `register`
share the serialized inventory update and conditional stop. Status reports
the configured and loaded roots. A missing Machine root points to registration.
A caller-owned list points to its configuration. Ordinary builds use Cargo
directly, and isolated builds bypass the cache.

## Delete — do not maintain

Removed the old dev-lf cache environment injection, build_cache.py exec-wrapper
and duplicate environment constants. Status reports only the configured
next-start timeout. Registration-triggered stop replaces idle-wait registration and its
checkout instructions. Keep the isolated natural-idle probe and fail-soft checkout.
The passive `refresh` command and launcher call are gone. Installation,
registration and the private probes share `write_inventory`.
