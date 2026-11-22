# Rebuild measurements

```bash
uv run python scripts/benchmarks/crate-rebuild/root_probe.py --sccache /path/to/sccache --output /tmp/lf-root-proof
uv run python scripts/benchmarks/crate-rebuild/probe.py --sccache /path/to/sccache --output /tmp/lf-cache-proof
uv run python scripts/benchmarks/crate-rebuild/cochange.py --revision HEAD --count 80
uv run python scripts/benchmarks/crate-rebuild/run.py --base 3fbf3d2ca3 --candidate HEAD --sccache /path/to/sccache --output /tmp/lf-build-comparison
uv run python scripts/benchmarks/crate-rebuild/report.py --input /tmp/lf-build-comparison --name loo-923 --output scripts/benchmarks/crate-rebuild/results
```

[LOO-650 results](results/loo-650.html) compare one application crate with the
storage and execution split. [LOO-923 results](results/loo-923.html) compare
that split with `loopflow-harness` beside storage. The report takes its cohort
count and cases from the run; `--cache-validation /tmp/lf-root-proof` adds the
root probe's receipts when it was run.

The probe builds the actual model, definition-content, terminal and storage crates in two separate
source/target roots in linked Git worktrees, with a shared `origin/main` ref.
It requires a cold miss, identical-root B hit, semantic-edit miss and correct checkout
identity from a path-sensitive fixture. A semantic Task-record edit must rebuild only storage. Storage reuse also covers its `test-support`
feature and byte-identical embedded database templates; copied databases still
acquire independent Machine identities. Root-refresh reuse is checked with a
fresh disposable target; existing dependency artifacts
can retain their earlier path hashes. It does not build the application. Use sccache with Rust basedir normalization and verbatim
environment-dependency tracking; 0.19.0 lacks these features. The first verified
upstream revision is `ca81ec2a2566efc623692741440d99c710268841`.

The full comparison clones committed baseline and candidate revisions into
independent source/target roots. Each label has its own cold-seeded private
cache. Five paired cohorts alternate candidate/base and base/candidate order.
Each starts with a fresh-target warm-cache build, then a command-help edit,
a shipping-message edit, a Task-record error-message edit and a Claude decoder
message edit, each timed through the `lf` link. The last case builds the unit-test
binary of the crate that owns the decoder (`--no-run`): that binary and its
dev-dependencies are built once outside the timing, a second decoder message is
edited, and the rebuild is timed. Each edit is
unique to its cohort; it cannot retrieve an earlier edited artifact. Each
revision's own workspace libraries decide what an edit must recompile, so the
two revisions may lay their crates out differently. Receipts
name edited files and actual workspace compiler hit/miss results, alongside
Cargo artifact freshness, aggregate cache statistics and executable link cost.
One serial Cargo job keeps per-invocation cache attribution unambiguous.

`summary.json` reports median/range elapsed seconds, client CPU and largest
process RSS over one sample per cohort (`--cohorts`, five by default) for each
case and revision. Cold seeds are calibration,
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

The root upkeep probe uses the same inventory/configuration function as
`scripts/dev-lf`. It creates a linked worktree after starting its private server,
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
