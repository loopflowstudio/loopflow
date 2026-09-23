# Native proof boundary — 2026-09-23

## Remaining work

Restore surface creation before judging the navigation/draft regression. The
failure reproduces without SwiftUI, the workspace, or user Ghostty configuration.
Do not change navigation ownership or weaken the native test to hide it. Once
surface creation succeeds, execute the configured navigation trial specified in
`review-slice.md`; the minimal probe establishes no focus or retention behavior.

## Discriminating evidence

- A separate AppKit/Metal process reports `screens=1 metal=true`. This refutes
  the earlier assertion that this process has no rendering environment; it does
  not establish a usable interactive application or permissioned UI test host.
- The final checked-in probe compiled successfully and independently reports
  `screens=1 metal=true`, followed by `surface=nil defaults=omitted`; its direct
  invocation exited 1. Thus screen/device discovery and surface failure also
  coexist in the same process, without relying on the separate capability read.
- The existing native regression, with `GHOSTTY_LOG=stderr`, reports
  `error(embedded_window): error initializing surface err=error.OutOfMemory`
  before its first surface assertion. Log:
  `/tmp/main-view-task-native-diagnostic.log`. The test failed; the surrounding
  command's final `tail` returned zero and is not a test success receipt.
- `native-surface-probe.m` links the exact SwiftPM artifact, creates an ordinary
  AppKit NSView, and invokes the C surface API with `/bin/cat`. It omits all
  user config loading unless explicitly passed an argument. Both a detached
  view and the checked-in window-attached version return nil with the same
  OutOfMemory error. Logs: `/tmp/loo291-ghostty-probe.log` and
  `/tmp/loo291-ghostty-mounted-probe.log`.
- The standalone probe excludes Swift Testing, SwiftUI layout/focus,
  GhosttyManager configuration/resource setup, and the navigation implementation.
  It does not distinguish an artifact defect from an environmental dependency
  inside Ghostty. An OutOfMemory error alone does not prove physical memory
  exhaustion. No dependency version or production configuration was changed.

## Reproduce from this checkout

```sh
GHOSTTY_LOG=stderr swift test --package-path swift -Xswiftc -gnone --jobs 4 \
  --filter WorkspaceNavigationProofTests/hiddenTerminalPreservesDraft

artifact=swift/.build/artifacts/swift/GhosttyKit/GhosttyKit.xcframework/macos-arm64_x86_64
xcrun clang -fobjc-arc -I "$artifact/Headers" scratch/native-surface-probe.m \
  "$artifact/libghostty.a" -framework AppKit -framework Metal \
  -framework QuartzCore -framework Carbon -framework CoreText \
  -framework IOKit -framework IOSurface -lc++ -o /tmp/loo291-ghostty-probe
GHOSTTY_LOG=stderr /tmp/loo291-ghostty-probe
```

The probe returns 1 for nil surface, 2 for library initialization failure,
3 for app initialization failure, and 0 only for successful surface creation.
It opens no visible window and reads/writes no planning or Session records.
Its only intended child is `/bin/cat`, freed with its surface.

The implement pass checkpointed the preceding review corrections with
`lf commit`; it changed no production code, Task status, PR publication, or
shared library artifact. Remaining Task scope is unchanged.
