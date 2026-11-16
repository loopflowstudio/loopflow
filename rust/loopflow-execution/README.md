# Shared execution

```sh
cargo build -p loopflow-execution --lib
cargo test -p loopflow-execution --lib task_graph::tests::
```

`engine` supplies typed operations; `ops` implements them. Provider conversations,
Task and Graph execution, acquisition above synchronous storage, shared readings,
Machine clients and native terminal lifetimes live here. The crate depends on
model, definitions, storage and terminal libraries and never on `loopflow`.

The application owns Clap, server dispatch and build provenance. Its entry points
supply `BuildIdentity` before execution. Landing selection takes the application's
canonical argv classifier; the adapter combines shared definition entries with
its command catalogue and invocation spellings. Captured Graph JSON and native
protocol values keep their wire shape.

`test-support` exposes shared fixtures, inspection and explicit overrides to dev
dependencies. CLI behavior keeps its ordinary defaults when Cargo unifies dev
features. Parser, help, dispatch and presentation assertions stay with the
application; execution's unit tests compile independently.

[Build and cache guidance](../../scripts/testing.md#builds) describes independent
worktree targets, optional cross-root reuse and isolated source debugging.
