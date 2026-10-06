# GhosttyKit embedder patches

```sh
uv run python scripts/loopflow-dev.py ghostty-build
```

Runs the patch's Zig tests, builds the pinned upstream revision with the patch,
and writes the framework under `swift/.build/local/` plus a versioned zip under
`swift/.build/artifacts/`. The command prints the SwiftPM checksum; it does not
publish the artifact.

The patch makes the terminal the owner of command blocks:

- The exit status a shell reports (`OSC 133;D;<code>`) is kept on the command's
  prompt row and survives reflow.
- A screen holds a text selection or a selected block, never both. Selecting
  either ends the other, and the selected block follows its command through
  reflow and scrollback.
- A prompt or command line that soft-wraps stays one prompt. Upstream starts a
  second prompt on the wrapped row, which splits the block.
- The surface C API lists visible blocks with their status and selection,
  selects the block at a row, and reads the selected block's command and output.

Loopflow draws block chrome from that list and holds no selection of its own.
Provider panes retain their native input behavior.

Publishing is a separately authorized upload of the zip to the `bin` R2 bucket
(`bin.loopflow.studio`) as `application/zip`, immutable, with R2 credentials
from Doppler. Download it back and compare its checksum with the build's, then
update the URL and checksum in `Package.swift`.
Use a new artifact version whenever the patch changes.

## Embedded macOS launch

`0002-embedded-login-session.patch` adds `macos-login-session`, default `true`.
Loopflow sets it `false`: the app supplies the environment and owns its PTYs,
so its children need no second login/accounting session. Shell strings retain
Bash's `--noprofile --norc -c 'exec -l …'` path, including login-shell startup.
Direct commands retain their argv. Cwd, environment overrides, shell integration,
process groups and PTY handling stay in Ghostty's existing implementation.
Standalone Ghostty keeps login(1), passwd lookup and hushlogin behavior by default.
The opt-out does not reset HOME/USER/LOGNAME/SHELL or print login banners.

For a local proof, temporarily replace only the GhosttyKit binary target in
`swift/Package.swift` with:

```swift
.binaryTarget(name: "GhosttyKit", path: ".build/local/GhosttyKit.xcframework"),
```

```sh
uv run python scripts/desktop_performance.py verify-fixture --mounted --lf target/debug/lf --output /tmp/ghostty-launch-proof
```

This runs the contained history/authority checks, then mounts real Ghostty
surfaces for explicit shell commands and the production companion-shell path.
It verifies login profile, environment, cwd, PTY input, terminal markers and
retained identity. Restore the published URL/checksum before checkpointing.
`--mounted` needs a native display; the default remains headless.

The October 6 local `GhosttyKit-4c83872-lf3.xcframework.zip` has SwiftPM checksum
`e490382b7f81f92b7bee8d303f8d8094b693d992f0e6fba2c820b7b370cf7ced`.
It is **not published**. The committed manifest still selects lf2, which does not
contain the launch repair. A release requires separately authorized publication
and verified manifest promotion; a local source checkpoint cannot activate it.
