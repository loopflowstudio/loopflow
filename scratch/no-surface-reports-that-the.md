# The running lf reports its own staleness

## Problem

A fix can be merged to main without reaching `/Users/jack/.local/bin/lf`.
`lf --version` does not expose that gap: `0.11.3` remained unchanged across the
four merged commits missing from the installed binary on 2026-07-17. Binary
mtime is also not evidence about the tree it compiled.

The exact answer is already embedded. `build.rs` emits
`LOOPFLOW_BUILD_SOURCE_REVISION`, and `build_info::source_revision()` reads it,
but no supported surface reports or compares it.

## Change

`lf doctor` reports `store.build_source_revision` and always appends a
`binary-freshness` check, even when the store cannot open.

The check:

1. finds the build checkout or the checkout containing the working directory;
2. refreshes `origin/main` with the explicit single-ref refspec
   `+refs/heads/main:refs/remotes/origin/main`;
3. proves the embedded revision exists before asking about ancestry;
4. classifies it as current, behind, off-main, or unprovable; and
5. when behind, prints every missing revision and subject oldest first.

A failed refresh or comparison is `Warn`/unprovable. It never falls back to a
possibly stale local ref and calls the binary current. The check reports only;
it never installs, rebuilds, or restarts anything.

`BuildFreshness` is deliberately four-way:

- `Current`: the embedded revision equals refreshed `origin/main`.
- `Behind`: it is an ancestor; the ordered range names missing merged work.
- `OffMain`: the object exists but is not on the upstream line.
- `Unprovable`: the object is absent or git cannot answer.

No revision is added to `BinaryProvenance`, `ChildProcessGeneration`, the
store, or Task status. This Task answers the running CLI question in
`lf doctor`; it does not build a fleet inventory.

## Proof

Classifier tests build a local `A -> B -> C` repository and cover:

- `C` is current;
- `A` is behind and lists `B`, then `C`;
- a feature commit off `B` is off-main; and
- an absent object or unresolved comparison is unprovable.

The supported-surface regression runs the real `lf doctor --json` binary and
asserts both `store.build_source_revision` and the `binary-freshness` check are
present. Its `PATH` contains no `git`, so the refresh fails closed without live
network state. Deleting `checks.push(check_binary_freshness())` makes the test
red.

## Done when

1. `lf doctor --json` reports the exact embedded source revision.
2. A successful refresh compares that revision with authoritative
   `origin/main`.
3. A behind build reports the missing count and ordered revisions/subjects.
4. Current, behind, off-main, and unprovable remain distinct; absence or git
   failure never reads as current.
5. No install, rebuild, restart, body-persistence, or Task-status path is added.

## Verification

- `cargo test -p loopflow --lib build_info::tests -- --nocapture`
- `cargo test -p loopflow --test doctor_tests -- --nocapture`
- `cargo clippy --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
