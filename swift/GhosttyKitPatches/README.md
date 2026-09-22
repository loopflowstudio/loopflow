# GhosttyKit command blocks

```sh
uv run python scripts/loopflow-dev.py ghostty-build
```

Builds the pinned upstream revision with these patches and writes the framework
under `swift/.build/local/` plus a versioned zip under `swift/.build/artifacts/`.
The command prints the SwiftPM checksum; it does not publish the artifact.

The patch exposes completed command block geometry and text through the surface
C API. Loopflow draws the block selection and copies its text without activating
Ghostty's character selection. Provider panes retain their native input behavior.

After publishing a new artifact, update the URL and checksum in `Package.swift`.
Use a new artifact version whenever the patch changes.
