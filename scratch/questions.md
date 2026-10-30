# 5b3da9 open questions

- `lf history --watch` now waits up to 5 minutes before an entry ages out of `--since`. New activity is still read as soon as it is committed.
- The Working Tasks sidebar gap (a started Task missing while `lf desktop view --plan --json` returned it) can't be checked headless. Desktop's `planning` part (`lf desktop plan --all --watch`) is the reader to check during demo.
- Classification judgment calls (`Commands::changes_work`): `lf wt switch` and `lf ci` record none; bare `lf pr`, `release notes` and `lf desktop do` (an action named at runtime) record. Each is one match arm to flip.
- Cargo's C objects in `libloopflow_ffi.a` target the SDK's macOS (26.2) while the app targets 26.0, so the app link prints deployment-version warnings. `MACOSX_DEPLOYMENT_TARGET=26.0` on the `loopflow-ffi` builds would quiet them.
- SwiftPM does not relink when only `libloopflow_ffi.a` changes. CI's restored SwiftPM cache can link a stale library into `swift-test` when a PR changes Rust but no Swift; `swift_cache.py`'s key could include `loopflow-ffi` and `read.rs`.
- `Read::Doctor` served in-app returns the report even when a check fails; through `lf` the app saw an error, since the command exits 1. The telemetry view now shows failed checks instead of an error.
- Desktop's planning refresh (`Write::PlanningRefresh`, in-process) runs once per shown Wave; a Git-planned repository with several Waves exchanges once per Wave each tick. Deduplicating per repository needs the transport, which Swift reads only from `PeerPlanning`.
- An in-app write's Process carries Desktop's PID and the `lf` argv it equals; nothing else marks it as Desktop's. Add a marker if history needs to tell them apart.
