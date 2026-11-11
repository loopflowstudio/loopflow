# iOS terminal on Ghostty: what is left

The Session terminal draws with GhosttyKit (`0006-ios-external-io.patch`).
Screenshots from a Simulator run fed a recorded stream are in `ios-ghostty/`.

## Before this can merge

- Publish `GhosttyKit-a60e9e2-lf8-ios1.xcframework.zip` by the workflow in
  `swift/GhosttyKitPatches/README.md`, then point `Package.swift` and
  `third_party/manifest.json` at it. Until then CI's
  `scripts/test_ios.py` step has no framework to link: `swift/ios.yml` reads
  `swift/.build/local/GhosttyKit.xcframework`, which only a local
  `ghostty-build` writes.
- Desktop moves to the same archive at that point. Its surface config struct
  gained a trailing field (`io_write_cb`); Desktop builds it with
  `ghostty_surface_config_new()`, so the field is null and the pty path is
  unchanged. Run the mounted Desktop fixture against the new archive before
  switching.

## Not checked

- A physical phone. The `ios-arm64` slice links and the device build succeeds;
  nothing has drawn on hardware.
- Scroll feel under a finger. The Simulator run scrolled by row through the
  same call the scroll view makes; inertia and rubber-banding were not felt.
- A hardware keyboard. Key translation is unit-tested for text and modifiers;
  no key was pressed on a device. Dead-key and input-method composition do not
  reach the terminal (the view takes key presses, not text input).
- VoiceOver reads the terminal as one element with no text.
- Text selection and copy by touch: nothing starts a selection.

## Checks

| Command | Result | Later |
|---|---|---|
| `uv run python scripts/loopflow-dev.py ghostty-build` | passed locally | on every patch change |
| `uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests` | passed locally | CI, once the archive is published |
| `xcodebuild … -destination 'generic/platform=iOS'` | built | CI does not build for a device |
