# GhosttyKit command blocks

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

Jack Heart authorized GhosttyKit artifact publication on October 6, 2026,
including future updates through this workflow; do not request approval again.
After the patch's relevant build and behavior checks pass, upload the zip to the
`bin` R2 bucket (`bin.loopflow.studio`) as `application/zip`, immutable, with R2
credentials from Doppler. Download it back and compare its checksum with the
build's, then update the URL and checksum in `Package.swift`.
Use a new artifact version whenever the patch changes.
