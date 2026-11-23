# LOO-921 — cross-worktree Rust caching and the edit-to-test loop

## Design and decisions

Install one pinned, local-disk sccache in personal Cargo configuration. Use
client-side compilation, one socket per pin, a shared cache directory, and
one target per worktree. Preserve unrelated personal Cargo settings and Git
hook content. No wrapper or cache settings in CI. The wrapper path carries
the revision because stock and pinned binaries both report version 0.19.0.

Git registration, the developer launcher and status share one root inventory.
The daemon reads it at startup. In 1h45m on November 16, the daemon had no
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
paired cohorts. Keep incremental off unless it wins the median three-edit
total in both app and execution. Concurrent-worktree capacity is unmeasured.

## Checks and remaining work

- November 16: private root and real-crate probes passed, including post-idle
  reuse, semantic edits, checkout identity and source-path observations. Four
  corrected seed commands passed. The original cohort exposed a cold
  incremental graph and a receipt wrapper basename that bypassed cc-rs.
  No measured warm rows from that run. Full observations and check commands:
  d52cf7179e:scratch/loo-921-cross-worktree-cache.md.
- November 22: a private pinned daemon shared by two linked Loopflow worktrees
  accepted a stop from B in 9 ms, with the same compiling rustls PID (42534) in
  A before and after. The replacement served B's root and A's execution test
  passed (1 passed, Cargo exit 0, 7m20s). The initial detector only matched
  `--emit=…`; a monitor on the same Cargo process group captured the real
  compiler's split `--emit …` arguments and executed the stop. Run the final
  integrated stop_probe.py through the installed Git hook after timing.
- Installed Machine hook: two detached worktrees at d52cf7179e ran plain
  `cargo build -p loopflow-definitions` via personal Cargo configuration.
  Each root was loaded after its first build. The second build took 1.88s,
  with 10 accumulated Rust hits and 0 misses on the replacement daemon.
  lsof still named its draining predecessor; status now reads the socket peer
  PID. Check named workspace hits during the final focused execution test.
- Run three warm pairs and produce results/loo-921.{html,json} and raw gzipped
  receipts. Record the resulting default, wall/CPU/RSS, hit/miss, link time
  and limitations in the report and scripts/testing.md.
- Verify the resulting Machine registration and fresh-root reuse on its
  normal path. Keep the 32 GiB disk reserve. The November 22 opening status
  showed 100/100 loaded and 10 GiB/10 GiB occupied.
- Gate owns documented_commands:: and the broader suites.

## Delete — do not maintain

Removed the old dev-lf cache environment injection, build_cache.py exec-wrapper
and duplicate environment constants. Status reports only the configured
next-start timeout. If the stop passes the real test, replace idle-wait
registration and the docs that tell a new checkout to wait. Keep the isolated
natural-idle probe and fail-soft checkout behavior.
