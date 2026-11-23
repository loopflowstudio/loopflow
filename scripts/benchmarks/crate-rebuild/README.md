# Rebuild measurements

```bash
uv run python scripts/benchmarks/crate-rebuild/root_probe.py --sccache /path/to/sccache --output /tmp/lf-root-proof
uv run python scripts/benchmarks/crate-rebuild/probe.py --sccache /path/to/sccache --output /tmp/lf-cache-proof
uv run python scripts/benchmarks/crate-rebuild/cochange.py --revision HEAD --count 80
uv run python scripts/benchmarks/crate-rebuild/run.py --base 3fbf3d2ca3 --candidate HEAD --sccache /path/to/sccache --output /tmp/lf-build-comparison
uv run python scripts/benchmarks/crate-rebuild/report.py --input /tmp/lf-build-comparison --cache-validation /tmp/lf-root-proof --output scripts/benchmarks/crate-rebuild/results
uv run python scripts/benchmarks/crate-rebuild/test_loop.py --output /tmp/lf-test-loop
uv run python scripts/benchmarks/crate-rebuild/report.py --test-loop /tmp/lf-test-loop --output scripts/benchmarks/crate-rebuild/results
```

[LOO-650 results](results/loo-650.html) include all five pairs and their raw receipts.
[LOO-921 results](results/loo-921.html) time a new worktree through a passing focused test.

## Edit-to-test loop

`test_loop.py` answers what a person waits for: `git worktree add`, one edit,
then a focused `cargo test` that passes. It uses the Machine's own Cargo
configuration from [`build_cache.py install`](../../testing.md#shared-compilation-cache)
(the pinned wrapper and the Machine's `jobs`), with a private cache directory
and socket plus a private root inventory so every counter belongs to the run. Both incremental settings get
a seed application test and execution test.
Switching `CARGO_INCREMENTAL` changes third-party artifact keys too; a seed in
only one mode leaves the other cold. Seeds are excluded from the samples.

| Sample | cache mode (`incremental = false`) | repeated-edit mode (`CARGO_INCREMENTAL=1`) |
|---|---|---|
| new worktree, application edit, `cargo test -p loopflow --lib <filter>` passes | ✓ | ✓ |
| two more application edits in that worktree, same test | ✓ | ✓ |
| execution edit in that worktree, `cargo test -p loopflow-execution --lib <filter>` passes | ✓ | ✓ |
| two more execution edits | ✓ | ✓ |
| new worktree, execution edit, execution test | ✓ | |

Each cohort runs every row once, alternating which mode goes first; three
cohorts is the minimum. Edits are unique to their cohort, so none can retrieve
an earlier edited artifact. Worktrees and targets are removed after their
samples. Cargo commands run one at a time. The existing repository disk reserve
is
checked before and during builds; crossing it stops only the experiment
process group and retains its receipts. No Machine cache or other checkout is
cleaned. After an exited run, `--resume` with the same output directory retains completed groups. An interrupted group starts
again with distinct edits and a fresh target; its earlier raw attempts move to
`abandoned-attempts/` so partially compiled artifacts cannot bias its samples.

Each sample records wall seconds, client CPU-seconds, the largest single
process RSS, the peak summed RSS of the Cargo process tree, and the daemon's
CPU-seconds beside the client's, not added to it. It records the command's Rust
hits, misses and non-cacheable compiles; for each workspace compiler its kind
(`lib`, `--test`, `bin`) and seconds; the test executables linked; Cargo
`--timings` durations for everything outside the workspace; jobs, toolchain and
load average before and after. A `--test` compile's seconds include its link.
With more than one job, a workspace crate's hit or miss is stated only where no
other compile overlapped it; the per-command counters are exact either way.

The report gives the median and range per row and mode, and the sum *new
worktree plus three edits* that decides whether incremental compilation stays
off by default. It states concurrent-worktree capacity as unmeasured: the peak
tree RSS and CPU-seconds describe one loop.

The shared Machine cache reached its 10 GiB cap during LOO-921 (2026-11-16).
The private warm-cache rows measure the cost of surviving entries, not their
survival rate under shared-cache eviction. The cap stays unchanged.

## Registration during a build

```bash
uv run python scripts/build_cache.py install
uv run python scripts/benchmarks/crate-rebuild/stop_probe.py --output /tmp/lf-stop-proof
```

The probe runs a focused execution test in one linked Loopflow worktree. While
a cacheable library is compiling, it adds another worktree through Git. Both
use the installed registration hook and pin with a private inventory, socket
and cache. Idle exit is disabled for this experiment. `stop.json` records the
compiler PID observed across registration, the original and replacement daemon
PIDs, whether the replacement loaded the second root, and the Cargo and test
outcomes. `cargo.log` holds the build output.

## Cache probes and the revision comparison

The probe builds the actual model, definition-content, terminal and storage crates in two separate
source/target roots in linked Git worktrees, with a shared `origin/main` ref.
It requires a cold miss, identical-root B hit, semantic-edit miss and correct checkout
identity from a path-sensitive fixture. A semantic Task-record edit must rebuild only storage. Storage reuse also covers its `test-support`
feature and byte-identical embedded database templates; copied databases still
acquire independent Machine identities. Root-refresh reuse is checked with a
fresh disposable target; existing dependency artifacts
can retain their earlier path hashes. It does not build the application. Use sccache with Rust basedir normalization and verbatim
environment-dependency tracking; 0.19.0 lacks these features.
`scripts/build_cache.py install` builds the verified revision and `status`
prints its path for `--sccache`.

The full comparison clones committed baseline and candidate revisions into
independent source/target roots. Each label has its own cold-seeded private
cache. Five paired cohorts alternate candidate/base and base/candidate order.
Each starts with a fresh-target warm-cache build, then a command-help edit,
a shipping-message edit, and a Task-record error-message edit. Each edit is
unique to its cohort; it cannot retrieve an earlier edited artifact. Receipts
name edited files and actual workspace compiler hit/miss results, alongside
Cargo artifact freshness, aggregate cache statistics and executable link cost.
One serial Cargo job keeps per-invocation cache attribution unambiguous.

`summary.json` reports median/range elapsed seconds, client CPU and largest
process RSS over five samples per case and revision. Cold seeds are calibration,
not cohort samples. The wrapper adds two cache-stat RPCs per workspace compiler;
that overhead is included. Runtime root/revision is checked in every source copy
outside timing. After the candidate seed, catalogue checks exercise origin/main
advancement, packed/loose refs and packaging without Git, outside timing.

Both revisions retain their own profiles and dependencies; inspect differences before attributing a
timing change to crate layout. It needs the source repository's `origin/main`
for migration ordering. Network fetches remain possible on cold builds.

The probes and full comparison own a private local cache and daemon and stop it on failure or
completion. They never reset the Machine's cache or share mutable targets. Raw
Cargo JSON, native time logs and cache receipts remain in the output directory.
The full comparison retains its source copies, targets and caches on failure
and prints their directory. After an exited run, pass `--resume` with the same
revisions, cache binary and output directory to continue its retained sources
and receipts. Completed timing samples are retained and their closures checked;
completed identity/catalogue checks are not repeated.
Nonfresh Cargo artifacts include cache retrieval; inspect sccache hit/miss deltas
to distinguish retrieval from compilation. Client-side compilation lets native
time include compiler and linker CPU in the client process tree; storage-daemon
overhead is excluded. These receipts do not establish total Machine CPU savings
or a safe concurrent build budget. The comparison and owner probe do not configure Machine admission budgets.

The root upkeep probe uses the same root list as `build_cache.py refresh`. It creates a linked worktree after starting its private server,
checks the expected miss while the old configuration remains active, waits for
that exact server's natural idle exit, then checks a hit in a fresh independent
target. It never sends a restart RPC or edits a root list. The tiny library checks
root reload; the owner probe separately checks the actual extracted crates.

The private daemon accepts frames up to 1 GiB: client-side cache uploads of
storage/application libraries exceed sccache's default 8 MiB IPC frame limit.
With that default, an upload can close the connection and lose the compilation's
statistics even though Cargo finishes successfully. Owner-only hit/miss checks
prevent dependency counters from concealing that failure.

The probe retains copies of the rlibs and their source-path strings. A cache hit
can retain the first checkout's absolute debug-source paths even when runtime
identity is correct. A successful cache counter check does not make those
paths resolvable; debug from `scripts/dev-lf --isolated-build`, which compiles
in `target/isolated` without the cache.
