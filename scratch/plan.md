# LOO-1061 · Mac terminal font weight

> “why is the font so bold in the terminal in loopflow app”

## Done when

Ordinary text in Loopflow for Mac’s Task shells and Session panes draws with Spline Sans Mono’s native stroke weight by default. ANSI bold and italic still look distinct. People who prefer extra stroke weight can set `font-thicken = true` in their Ghostty config and reopen Loopflow.

## Why it looks bold

`GhosttyManager.loopflowConfig` enables `font-thicken` for every Mac surface before loading personal configuration. The pinned Ghostty defaults are `false` and strength `255`. With thickening enabled, Ghostty passes that setting to the CoreText glyph renderer for ordinary text too. Spline Sans Mono became the default family in the next change, which retained the stroke setting. The thickening PR records that its rendered terminal had yet to be checked visually. [LOO-1014 and PR #2011](https://github.com/loopflowstudio/loopflow/pull/2011) · [PR #2016](https://github.com/loopflowstudio/loopflow/pull/2016) · [Mac defaults](../swift/LoopflowMac/Services/Ghostty/GhosttyManager.swift#L129) · [Pinned Ghostty defaults](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/config/Config.zig#L343-L355) · [CoreText rasterization](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/font/face/coretext.zig#L477-L483)

A headless probe linked to the checkout’s pinned `GhosttyKit-a60e9e2-lf8` artifact read the effective value after finalizing, cloning and adding sample colors. Ghostty alone resolved to `false`, the current Loopflow defaults to `true`, and current defaults with an explicit personal opt-out to `false`. Removing the override resolved to `false`, or `true` with a personal opt-in. Strength resolved to `255` in all five cases with zero config diagnostics. This measured configuration readback. The rendered difference in an actual Loopflow pane is the demo check.

## Design

The choice for review is to delete `font-thicken = true` from the Mac scalar defaults and let the bundled Ghostty config own the default. Lowering `font-thicken-strength` would still smooth regular strokes. Switching to a lighter font would change glyph metrics and fallback while leaving smoothing active. The current option already lets people select their preferred weight. [Ghostty option reference](https://ghostty.org/docs/config/reference#font-thicken)

The effective configuration passed to `ghostty_app_new` and `ghostty_app_update_config` supplies both Session and shell surfaces. Its order is Ghostty defaults, Loopflow scalar defaults, personal Ghostty files, embedded settings, and the Loopflow color overlay. An omitted option inherits `false`, and a valid personal boolean takes precedence across theme changes. Ghostty owns file parsing and its diagnostics for invalid values. The embedded family list keeps a configured family first and adds Spline Sans Mono for fallback. The color overlay controls the palette. Minimum contrast is a separate scalar default. The iPhone uses its own Ghostty initialization, which already inherits the renderer default. [Mac configuration and surface creation](../swift/LoopflowMac/Services/Ghostty/GhosttyManager.swift) · [Color overlay](../swift/Loopflow/Design/GhosttyThemeOverlay.swift) · [iPhone configuration](../swift/LoopflowiOS/GhosttySurface.swift)

## Slices and gate

1. Exercise the production Mac config texts and `GhosttyThemeConfiguration.candidate` with the pinned library. Add `terminalStrokeWeightFollowsGhosttyConfig` beside the compiled theme readback tests, using temporary personal config text in the same load order. With light and dark candidates, read `font-thicken` as a `Bool` through `ghostty_config_get`. The absent case fails against the current production default. Deleting its one setting makes the absent and explicit `false` cases read `false`, and explicit `true` reads `true`. The focused headless gate is `scripts/test_desktop.sh -Xswiftc -gnone --filter terminalStrokeWeightFollowsGhosttyConfig`. [Existing configuration tests](../swift/LoopflowTests/ThemeTests.swift#L375)
2. Document the default and Ghostty opt-in in the terminal section of the Mac guide. The demo opens this checkout via `uv run python scripts/loopflow-dev.py run`. In a Task shell with no personal `font-thicken` setting, run the sample below in Loopflow Light and Loopflow Dark. The first line uses lighter native strokes than the base build, and the second and third remain visibly bold and italic. A live Session shows the same weight. To compare the opt-in, setting `font-thicken = true` and reopening the app restores the thicker strokes on the same sample. [Mac terminal guide](../swift/README.md#L767)

   ```sh
   printf 'Regular  Il1  0O  -> != =>\n\033[1mBold     Il1  0O\033[0m\n\033[3mItalic   Il1  0O\033[0m\n'
   ```
