# Rebuild measurements

```bash
uv run python scripts/benchmarks/crate-rebuild/probe.py --sccache /path/to/sccache --output /tmp/lf-cache-proof
uv run python scripts/benchmarks/crate-rebuild/cochange.py --revision HEAD --count 80
uv run python scripts/benchmarks/crate-rebuild/run.py --base 3fbf3d2ca3 --candidate HEAD --sccache /path/to/sccache --output /tmp/lf-build-comparison
```

The probe builds the actual model, definition-content, terminal and storage crates in two separate
source/target roots. It requires a cold miss, identical-root B hit, semantic-edit
miss and correct checkout identity from a path-sensitive fixture. A semantic Task-record
edit must rebuild only storage. Storage reuse also covers its `test-support`
feature and byte-identical embedded database templates; copied databases still
acquire independent Machine identities. Root-refresh reuse is checked with a
fresh disposable target; existing dependency artifacts
can retain their earlier path hashes. It does not build the application. Use sccache with Rust basedir normalization and verbatim
environment-dependency tracking; 0.19.0 lacks these features. The first verified
upstream revision is `ca81ec2a2566efc623692741440d99c710268841`.

The full comparison clones the committed baseline and candidate into disposable
copies. It runs serial cold A, fresh-target warm B, and a real shipping-message
edit in B, then a Task-record error-message edit on top of it. The Task edit
follows its source across crate moves; each receipt names the edited file. Each
case measures only its latest edit, with one Cargo job unless selected otherwise.
After timing, the candidate also checks Cargo catalogue refresh across origin/main-only
advancement, packed references, a new loose reference and a source package without Git.
Those checks are separate from the timing receipts.

Both revisions retain their own profiles and dependencies; inspect differences before attributing a
timing change to crate layout. It needs the source repository's `origin/main`
for migration ordering. Network fetches remain possible on cold builds.

Both commands own a private local cache and daemon and stop it on failure or
completion. They never reset the Machine's cache or share mutable targets. Raw
Cargo JSON, native time logs and cache receipts remain in the output directory.
The full comparison retains its source copies, targets and caches on failure
and prints their directory, so a measurement error need not discard built work.
Nonfresh Cargo artifacts include cache retrieval; inspect sccache hit/miss deltas
to distinguish retrieval from compilation. Client-side compilation lets native
time include compiler and linker CPU in the client process tree; storage-daemon
overhead is excluded. These receipts do not establish total Machine CPU savings
or a safe concurrent build budget. The commands do not configure shared daemon
root refresh or Machine admission budgets.

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
