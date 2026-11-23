# LOO-1061 · Mac terminal font weight

> “why is the font so bold in the terminal in loopflow app”

## Done when

Ordinary text in Loopflow for Mac’s Task shells and Session panes uses Spline Sans Mono’s native stroke weight by default. The bundled bold and italic faces still distinguish ANSI styles. A personal `font-thicken = true` applies to both kinds of pane after reopening Loopflow. Previewing, applying or cancelling a color theme keeps the same terminal font settings.

## Why it looks bold

`GhosttyManager.loopflowConfig` previously enabled `font-thicken` before loading personal configuration. Ghostty passes this setting to CoreText when drawing glyphs, including regular ones. Spline Sans Mono became the default family in the next change and inherited the stroke override. The PR that added thickening recorded its Mac rendering as visually untried. [PR #2011](https://github.com/loopflowstudio/loopflow/pull/2011) · [PR #2016](https://github.com/loopflowstudio/loopflow/pull/2016) · [Pinned Ghostty defaults](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/config/Config.zig#L343-L355) · [CoreText rasterization](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/font/face/coretext.zig#L477-L483)

A headless probe against the pinned `GhosttyKit-a60e9e2-lf8` read the effective value after finalizing, cloning and adding sample colors. Ghostty alone returned `false`. The shipped Loopflow defaults returned `true` and a personal opt-out returned `false`. With the override removed, absence returned `false` and a personal opt-in returned `true`. Strength read `255` in all five cases with zero diagnostics.

## Design

One terminal font configuration serves all Mac panes. The running program chooses styles per cell. Ghostty owns the stroke default and reads any personal override at app startup. The change removes `font-thicken = true` from `GhosttyManager.loopflowConfig`. Changing fonts would change glyph metrics and fallback. Reducing the strength would still enable thickening at its lowest value. [Ghostty option reference](https://ghostty.org/docs/config/reference#font-thicken)

The startup baseline is Ghostty defaults, Loopflow scalar defaults, personal Ghostty files, then embedded settings. The embedded settings append Spline Sans Mono after any personal family and clear Ghostty’s `theme` reference before finalization so Appearance supplies the palette. The baseline feeds a validated `GhosttyThemeConfiguration.candidate` for `ghostty_app_new` and `ghostty_app_update_config`. Each candidate clones the same baseline and overlays the selected palette. Shells and Sessions share this Ghostty app, with color previews updating the existing panes. The effective font is derived configuration. There is no Task, Session or theme record to migrate. [Mac baseline and app calls](../swift/LoopflowMac/Services/Ghostty/GhosttyManager.swift) · [Candidate](../swift/LoopflowMac/Design/GhosttyThemeConfiguration.swift) · [Color overlay](../swift/Loopflow/Design/GhosttyThemeOverlay.swift)

Appearance previews live colors for the app and its terminals. `AppearanceStore` stages a `ThemeDefinition` and the selection. Apply saves the choice in `~/.lf/config.yaml` and any staged palette files under `~/.lf/themes/`. Cancel restores the applied palette. The chooser’s terminal line is a SwiftUI color sample, and the Mac guide directs font checks to an open pane. When a theme file contains `font-thicken`, the importer lists the omission and previews the supported colors. Unreadable colors keep the last valid palette and disable Apply. To apply the font setting, put the line in the personal Ghostty configuration and reopen Loopflow. [Appearance store](../swift/LoopflowMac/Design/AppearanceStore.swift) · [Theme import](../swift/Loopflow/Design/ThemeFile.swift#L133) · [Chooser sample](../swift/LoopflowMac/Views/ThemeChooser.swift#L185)

## Implementation check

`scripts/test_desktop.sh -Xswiftc -gnone --filter terminalStrokeWeightFollowsGhosttyConfig` failed on the old default with no personal setting in Light, Dark and Light again. After removing the override, all three cases passed. Ghostty read native weight with the setting absent or false and thickened strokes with it true at each theme selection.

`scripts/test_desktop.sh --skip-build -Xswiftc -gnone --filter 'themeOverridesColorsAndKeepsFontSettings|boundedImportRetainsColorsAndReportsOmissions|unreadableEditsKeepTheLastAppearanceAndDisableApply|applyChangesOnlyTheThemeAndCancelLeavesTheFileUntouched|embeddedConfigBindsPromptJumps|defaultConfigurationLoadsAfterInitialAppearance'` passed six headless tests on the built branch. The theme test checks absent, false and true across all built-in palettes and two imports.

Mounted QA captured a Task shell in Loopflow Light and Dark. The same branch build with `font-thicken = true` reproduced the removed default after relaunch. Regular text measured 33% more summed pixel contrast in each palette, and the Light crops for an absent setting and explicit `false` were identical. The native Light sample showed distinct bold and italic text. [QA captures](./qa.html)

## Finished-change review

Keep the built change. Font choices belong to the shared Ghostty baseline and app and terminal colors belong to Appearance. A chooser rendered with Ghostty would duplicate the live pane. Carrying font settings in theme files would make them change with the palette. The guide explains the existing color sample and the personal opt-in. [Concept review](./concept-review.html) · [Terminal font](../swift/README.md#L789) · [Appearance sample](../swift/README.md#L318)

The Session stand-in printed nothing, and the screen locked before the mounted chooser checks. At ship review with a display, show the sample below in a Session, then preview and cancel a color change with both panes visible. Import a valid theme containing `font-thicken = true`, check the omission message, and apply the palette. Reopen with a personal opt-in and confirm both panes get thicker strokes with the saved palette. If text still looks heavy with thickening off, compare at the same size, scale and palette to isolate the selected face or renderer. The mounted Shell comparison and automated checks support advancing from concept review.

```sh
printf 'Regular  Il1  0O  -> != =>\n\033[1mBold     Il1  0O\033[0m\n\033[3mItalic   Il1  0O\033[0m\n'
```
