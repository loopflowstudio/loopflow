# GhosttyKit embedder patches

```sh
uv run python scripts/loopflow-dev.py ghostty-build
```

Requires Zig 0.16.0. Runs the patches' Zig tests, builds the pinned upstream revision,
and writes the framework under `swift/.build/local/` plus a versioned zip under
`swift/.build/artifacts/`. The command prints the SwiftPM checksum; it does not
publish the artifact. Packaging normalizes the upstream static archive to
`libghostty.a` and updates its plist paths so SwiftPM links it.

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

Jack Heart authorized GhosttyKit artifact publication on October 6, 2026,
including future updates through this workflow; do not request approval again.
After the patch's relevant build and behavior checks pass, upload the zip to the
`bin` R2 bucket (`bin.loopflow.studio`) as `application/zip`, immutable, with R2
credentials from Doppler. Download it back and compare its checksum with the
build's, then update the URL and checksum in `Package.swift`.
Use a new artifact version whenever the patch changes.

## Bounded text

```sh
# In the patched Ghostty source; terminal-only proof, including on Linux.
zig build test-lib-vt -Demit-lib-vt=true -Dtest-filter='bounded text'
```

`0004-bounded-text.patch` adds `ghostty_surface_read_text_bounded`. Supply a
selection for viewport or scrollback, or NULL for the current text/command-block
selection. Supply the destination buffer and its byte capacity. The result gives
bytes written and whether content was truncated; it is not NUL-terminated.
An empty terminal/selection succeeds with zero bytes, while an invalid explicit
selection fails. The prefix always ends at a complete UTF-8 scalar.

The existing `ScreenFormatter` writes directly to fixed caller storage and stops
when it fills. Neither the reader nor its formatter allocates a full string or
pin map. The renderer lock covers selection resolution and extraction. Reading
an invalidated command-block selection no longer releases its tracked pin:
selection changes, reset and teardown keep that ownership. Clipboard/Quick Look
retain their existing unbounded API and viewport metadata.

`Package.swift` selects the published `GhosttyKit-a60e9e2-lf3.xcframework.zip`.
Its public download matches SwiftPM checksum
`dfa0e65b216cccbee132b1d579bc2bd646fda34b2822fe8e00b0306b70bada7f`.
The October 9 macOS build passed 270 patch tests (four skipped) and exported the
bounded reader for arm64 and x86_64. Reset now releases command-block selection
directly; the prior suite failed because passive reads no longer did that cleanup.
Desktop reads viewport, full scrollback or current selection into fixed storage,
then copies only the written UTF-8 prefix. Empty selection succeeds. Native pane
fixtures compile but require a display; headless checks do not prove mounted reads.

On Linux, also compile the embedded C entry point (the terminal-only test does
not compile it):

```sh
zig build -Dapp-runtime=none -Dfont-backend=fontconfig_freetype -Drenderer=opengl -Demit-lib-vt=false
nm -D zig-out/lib/ghostty-internal.so | grep ' T ghostty_surface_read_text_bounded$'
```

Prepare lazy dependencies with networking enabled before an isolated build.
Use the discovery-capable Fontconfig backend: `freetype` alone does not compile
upstream's embedded app. This verifies Linux libraries, not a macOS framework,
native surface reads or publication readiness.

## Program Status

`0003-program-status.patch` forwards validated OSC 7501 reports from Ghostty's
terminal parser through `GHOSTTY_ACTION_PROGRAM_STATUS` in the embedded runtime.
It adds no parser, record store, notification policy, or process authority.
The standalone runtime does not answer or forward these reports.

The action union's `program_status` member points to
`ghostty_action_program_status_s`. Copy it and every string before returning from
the callback; the terminal thread copied the validated report before parser reuse.

| Field | Values |
| --- | --- |
| `event` | `GHOSTTY_PROGRAM_STATUS_REPORT`, `PROMPT`, `RESET`, `EXIT` |
| `state` | idle 0, working 1, done 2, blocked 3, error 4, clear 5; -1 for boundaries |
| `kind` | permission 0, question 1, auth 2; -1 when absent |
| `progress` | 0–100; -1 when absent |
| `id` | NUL-terminated UTF-8; empty string is the root record |
| `app`, `title`, `msg` | Optional NUL-terminated UTF-8; NULL means absent |

Text is already decoded, control-checked by the upstream parser, and untrusted.
Display it literally. Prompt events accompany OSC 133 A/P, reset accompanies RIS,
and exit precedes the attached child's existing exit handling. The app owns
record lifetimes: drop working/blocked at prompt or exit, clear on reset, and
dismiss retained done/error on keyboard interaction. Focus is not interaction.
DECSTR and alternate-screen switches do not send reset events.

The embedded runtime answers `OSC 7501;?` using the request's BEL or ST terminator.
The action is appended to the existing enum; its pointer fits the existing
action union. The forwarding patch is separable from command-block and launch
patches and can be removed when upstream supplies the equivalent action.

Headless checks exercise the upstream parser, decoded payload ownership after
parser reuse, absent fields, action-union conversion and the exported C layout.
They also retain the command-block/reflow and embedded launch tests. They do not
prove mounted app presentation or Session observation.

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

The published `GhosttyKit-a60e9e2-lf2.xcframework.zip` has SwiftPM checksum
`c2add4ae90d1e8f3394fb19b8d6f28b76318cbb497509c7534f3f6e8a59826a9`.
The October 7 clean headless build passed the patch/protocol checks and produced
both arm64 and x86_64 libraries. A full public download with curl matched the
checksum; Python urllib received HTTP 403. Swift imported the new action header.
`Package.swift`, packaged shell/terminfo resources and the runtime resource
revision select the same upstream commit.

This revision also changes the upstream clipboard callback ABI: reads return
`ghostty_clipboard_read_result_e` and receive MIME candidates and a confirmation
flag; completion takes `ghostty_clipboard_complete_s`; confirmation receives
`ghostty_clipboard_confirm_s`. The app adopts those signatures and preserves length-delimited clipboard text.
No mounted surface, pane presentation or Session observation is established by
the framework build.
