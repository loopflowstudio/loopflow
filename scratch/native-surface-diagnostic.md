# Native surface restoration — 2026-09-23

## Remaining proof

Surface creation and the retained draft/focus fixture now pass. The configured
navigation trial in `review-slice.md` remains required: actual planning and
Session access, split/layout/scroll retention, and human-resolution reconciliation.
Timer rendering has not received a visual or performance verdict. No external
trial, publication, landing, or Task completion is established here.

## Cause and correction

The isolated AppKit probe sees one screen and a Metal device, but
`CVDisplayLinkCreateWithActiveCGDisplays` returns `-6661`
(`kCVReturnInvalidArgument`). Ghostty maps any failure of this call to
`error.OutOfMemory`. The error did not establish physical memory exhaustion.
The pinned binary's disassembly contains this call and its error branch;
the local Ghostty source explains the mapping in
`pkg/macos/video/display_link.zig` and conditional creation in
`src/renderer/generic.zig`. The lower CoreVideo rejection's cause remains unknown.

The same probe and artifact, with no user configuration, fail by default
(exit 1) and create a surface with only `window-vsync = false` (exit 0).
Logs: `/tmp/loo291-display-link-default.log`, `/tmp/loo291-no-vsync.log`.
An LLDB launch of this disposable probe was denied by macOS attach permissions;
no debugger trace is claimed and no user process was attached.

`GhosttyManager` now checks the same CoreVideo capability once at initialization.
On failure it logs the status and configures Ghostty's existing timer renderer.
On success it leaves the user's vsync setting intact. The probe is released;
no second rendering loop, artifact replacement, user config write, or Session
authority is introduced. The exact API check produces a macOS 15 deprecation
warning because the pinned library still uses that CoreVideo API.

Restored surface creation exposed a focus bug: a request made before attachment
remained pending in the SwiftUI coordinator, so resize stole a search field's
first responder. The retained native view now owns that request, applies it on
attachment/transition, and releases focus when hidden. The coordinator is deleted.

## Focused receipts

After the rendering correction alone, the unchanged native fixture created its
surface but failed at line 56: resize changed first responder from the field
editor to the terminal. `/tmp/loo291-timer-native-proof.log`, exit 1.

After the focus correction:

```sh
GHOSTTY_LOG=stderr swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter WorkspaceNavigationProofTests/hiddenTerminalPreservesDraft
```

One test passed, exit 0; `/tmp/loo291-timer-focus-proof.log`. This uses the real
AppKit host, Ghostty surface, and `/bin/cat` PTY. It proves unfinished input
survives hiding/restoring the same surface, hidden input focus is released,
explicit focus works, and resize leaves search input focused. No assertions were
weakened or skipped. It does not prove configured provider or complete navigator
behavior.

The previous independent counterexample now also passes:

```sh
GHOSTTY_LOG=stderr swift test --package-path swift -Xswiftc -gnone --jobs 4 --skip-build --filter GhosttyTerminalInputTests/releaseSurfaceIsWindowLocal
```

One test passed, exit 0; `/tmp/loo291-timer-window-proof.log`. Releasing a surface
in one window leaves the other window's same-identity surface alive. No Swift
source or test edits followed these passes; no broader gate was run.

## Reproduce the isolated boundary

```sh
artifact=swift/.build/artifacts/swift/GhosttyKit/GhosttyKit.xcframework/macos-arm64_x86_64
xcrun clang -fobjc-arc -Wno-deprecated-declarations -I "$artifact/Headers" scratch/native-surface-probe.m "$artifact/libghostty.a" -framework AppKit -framework Metal -framework QuartzCore -framework Carbon -framework CoreText -framework CoreVideo -framework IOKit -framework IOSurface -lc++ -o /tmp/loo291-ghostty-probe
GHOSTTY_LOG=stderr /tmp/loo291-ghostty-probe
printf 'window-vsync = false\n' > /tmp/loo291-no-vsync.conf
GHOSTTY_LOG=stderr /tmp/loo291-ghostty-probe /tmp/loo291-no-vsync.conf
```

The optional argument loads exactly that configuration file, never user defaults.
The probe returns 1 for nil surface, 2 for library initialization failure,
3 for app initialization failure, and 0 for successful surface creation.
It opens no visible window and reads/writes no planning or Session records.
Its `/bin/cat` child is freed with its surface. Earlier nil-surface receipts in
`navigation-proof.md` remain historical evidence, superseded by these results.

Compression follow-up: checking the existing Task terminal caller exposed one
lost distinction between disabled and merely unselected views. The extended
native fixture reproduced manual focus being cleared on resize; the native
focus update now receives both inputs and preserves manual focus while enabled.
That same fixture passes after correction. `compress.md` records the exact
before/after commands; these supersede the earlier focus receipt for current
source. No rendering configuration or ownership change accompanied this fix.
