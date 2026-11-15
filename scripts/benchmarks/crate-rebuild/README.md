# Rebuild measurements

```bash
uv run python scripts/benchmarks/crate-rebuild/probe.py --sccache /path/to/sccache --output /tmp/lf-cache-proof
uv run python scripts/benchmarks/crate-rebuild/cochange.py --revision HEAD --count 80
uv run python scripts/benchmarks/crate-rebuild/run.py --base 3fbf3d2ca3 --candidate HEAD --sccache /path/to/sccache --output /tmp/lf-build-comparison
```

The probe builds the actual model, definition-content and terminal crates in two separate
source/target roots. It requires a cold miss, identical-root B hit, semantic-edit
miss and correct checkout identity from a path-sensitive fixture. Root-refresh
reuse is checked with a fresh disposable target; existing dependency artifacts
can retain their earlier path hashes. It does not build the application. Use sccache with Rust basedir normalization and verbatim
environment-dependency tracking; 0.19.0 lacks these features. The first verified
upstream revision is `ca81ec2a2566efc623692741440d99c710268841`.

The full comparison clones the committed baseline and candidate into disposable
copies. It runs serial cold A, fresh-target warm B, and a real shipping-message
edit in B, with one Cargo job unless selected otherwise. Both revisions retain
their own profiles and dependencies; inspect differences before attributing a
timing change to crate layout. It needs the source repository's `origin/main`
for migration ordering. Network fetches remain possible on cold builds.

Both commands own a private local cache and daemon and stop it on failure or
completion. They never reset the Machine's cache or share mutable targets. Raw
Cargo JSON, native time logs and cache receipts remain in the output directory.
Nonfresh Cargo artifacts include cache retrieval; inspect sccache hit/miss deltas
to distinguish retrieval from compilation. Client-side compilation lets native
time include compiler and linker CPU in the client process tree; storage-daemon
overhead is excluded. These receipts do not establish total Machine CPU savings
or a safe concurrent build budget. The commands do not configure shared daemon
root refresh or Machine admission budgets.

The probe retains copies of the rlibs and their source-path strings. A cache hit
can retain the first checkout's absolute debug-source paths even when runtime
identity is correct. Inspect these before enabling automatic cross-root reuse;
a successful cache counter check alone does not settle debugger source lookup.
