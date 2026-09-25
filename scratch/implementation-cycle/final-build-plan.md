# Remaining native build boundary

After the Flow and Comments slices and their focused reviews settle, run the
existing resource-checked, supervised `loopflow` compile plan alone:

```python
from scripts.test import build_plan, run_plans
raise SystemExit(run_plans([p for p in build_plan([], False, {"loopflow"}) if p.run]))
```

Execute with `uv run python`; do not invoke a broader affected-suite plan merely
to obtain the Xcode compile. Preserve preflight failures and use only supported
resource recovery. This is the existing runner, not a preflight bypass.

Source inspection confirms `swift/project.yml` includes the entire `LoopflowMac`
directory except Info.plist/GhosttyResources, and all `LoopflowTests`. Newly added
Swift views therefore enter Xcode generation through existing globs. That source
fact does not establish fallback compilation; the compile above remains pending.
SwiftPM's focused native tests link Ghostty, whereas Xcode's app/test target
checks the terminal fallback. Neither substitutes for configured provider/demo
or human acceptance.

## Build resource correction before cycle 4 compilation

The read-only preflight at `/tmp/loo291-native-build-resources.log` reported this
checkout's generated build roots at 15.4 GiB, above 12 GiB. Supported `--recover`
correctly retained active worktrees and left the same block. Root inventory:
Rust target 12 GiB (8.6 GiB debug/deps), Swift build 3.6 GiB; free disk 199 GiB.

Parent made an explicit retention choice for its own regenerable package builds:
`cargo clean --manifest-path rust/loopflow/Cargo.toml --target-dir
/Users/jack/src/loopflow.main-view-task/target -p loopflow --profile dev`.
Cargo's prior dry run selected 2,344 files/8.4 GiB; the bounded cleanup uses the
package manager, not a broad directory deletion. It does not change source,
runtime/PM data, other worktrees, dependency caches or the resource budget.
Prior test logs and source-attributed receipts remain historical evidence even
though generated executables are rebuilt by the next compilation. The final
supervised build must still pass its ordinary preflight after new compilation.
Receipts: `/tmp/loo291-build-clean{,-dev-preview}.log`.
The actual command reports 2,330 files/8.2 GiB removed (exit 0); this differs
slightly from the preview and is the consequential result, not an 8.4-GiB claim.
The next read-only preflight passes: 7.2 GiB for this checkout, 207 GiB free;
`/tmp/loo291-native-build-resources-after-clean.log`, exit 0. Final compilation
still needs its own ordinary preflight/postflight after the remaining source work.
