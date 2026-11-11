# iOS terminal on Ghostty: what is left

The Session terminal draws with GhosttyKit (`0006-ios-external-io.patch`).
Screenshots from a Simulator run fed a recorded stream are in `ios-ghostty/`.

## Before this can merge

- Run the mounted Desktop fixture against the lf8 archive (`Package.swift` now
  pins it for both apps): `uv run python scripts/desktop_performance.py
  verify-fixture --mounted --lf target/debug/lf`. Its surface config struct gained
  a trailing field (`io_write_cb`); Desktop builds it with
  `ghostty_surface_config_new()`, so the field is null and the pty path is
  unchanged. Needs a display.

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
| `xcodebuild … -destination 'generic/platform=iOS'` | built | CI does not build for a device |
| `swift build --product LoopflowMac` + `swift test --filter "Ghostty\|OpenSourceLicenses"` on the pinned lf8 | built, 24 passed | gate |
| `uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests` on the pinned lf8 | passed | CI |
| `uv run pytest python/tests/test_third_party.py python/tests/test_app_bundle.py` | 13 passed | gate |
| `scripts/third_party.py package` into an empty directory | 5.1 MB: notices, z2d and option-ext source | release |
