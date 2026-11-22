# LOO-1061 · Mac terminal font weight

> “why is the font so bold in the terminal in loopflow app”

## Done when

Ordinary text in Loopflow for Mac’s Task shells and Session panes uses Spline Sans Mono’s native stroke weight by default. The bundled bold and italic faces still distinguish ANSI styles. A personal `font-thicken = true` applies to both kinds of pane after reopening Loopflow. Previewing, applying or cancelling a color theme keeps the same terminal font settings.

## Why it looks bold

`GhosttyManager.loopflowConfig` previously enabled `font-thicken` before loading personal configuration. Ghostty passes this setting to CoreText when drawing glyphs, including regular ones. Spline Sans Mono became the default family in the next change and inherited the stroke override. The PR that added thickening recorded its Mac rendering as visually untried. [PR #2011](https://github.com/loopflowstudio/loopflow/pull/2011) · [PR #2016](https://github.com/loopflowstudio/loopflow/pull/2016) · [Pinned Ghostty defaults](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/config/Config.zig#L343-L355) · [CoreText rasterization](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/font/face/coretext.zig#L477-L483)

A headless probe against the pinned `GhosttyKit-a60e9e2-lf8` read the effective value after finalizing, cloning and adding sample colors. Ghostty alone returned `false`. The shipped Loopflow defaults returned `true` and a personal opt-out returned `false`. With the override removed, absence returned `false` and a personal opt-in returned `true`. Strength read `255` in all five cases with zero diagnostics. The stroke difference in a mounted pane still needs a visual check.

## Design

One terminal font configuration serves all Mac panes. The running program chooses styles per cell. Ghostty owns the stroke default and reads any personal override at app startup. The change removes `font-thicken = true` from `GhosttyManager.loopflowConfig`. Changing fonts would change glyph metrics and fallback. Reducing the strength would still enable thickening at its lowest value. [Ghostty option reference](https://ghostty.org/docs/config/reference#font-thicken)

The startup baseline is Ghostty defaults, Loopflow scalar defaults, personal Ghostty files, then embedded settings. The embedded family list adds Spline Sans Mono after any personal family. The baseline feeds a validated `GhosttyThemeConfiguration.candidate` for `ghostty_app_new` and `ghostty_app_update_config`. Each candidate clones the same baseline and overlays the selected palette. Shells and Sessions share this Ghostty app, with color previews updating the existing panes. The effective font is derived configuration. There is no Task, Session or theme record to migrate. [Mac baseline and app calls](../swift/LoopflowMac/Services/Ghostty/GhosttyManager.swift) · [Candidate](../swift/LoopflowMac/Design/GhosttyThemeConfiguration.swift) · [Color overlay](../swift/Loopflow/Design/GhosttyThemeOverlay.swift)

Appearance is the live color preview for the app and its terminals. `AppearanceStore` handles selection and cancellation, `ThemeDefinition` carries app and terminal colors, and the selected theme lives in `~/.lf/config.yaml`. The chooser’s terminal line is a SwiftUI color sample. Describe it that way in the Mac guide and direct font checks to an open terminal pane. An imported theme that contains `font-thicken` reports that the key was omitted. To recover, put the line in the Ghostty configuration loaded at launch and reopen Loopflow. Personal font settings, the selected theme and its palette stay independent. [Appearance store](../swift/LoopflowMac/Design/AppearanceStore.swift) · [Theme import](../swift/Loopflow/Design/ThemeFile.swift#L133-L136) · [Chooser sample](../swift/LoopflowMac/Views/ThemeChooser.swift#L185-L225)

## Implementation check

`scripts/test_desktop.sh -Xswiftc -gnone --filter terminalStrokeWeightFollowsGhosttyConfig` failed on the old default with no personal setting in Light, Dark and Light again. After removing the override, all three cases passed. Ghostty read native weight with the setting absent or false and thickened strokes with it true at each theme selection.

The Mac guide describes the personal opt-in, reopening Loopflow and checking typography in a real pane. [Terminal font](../swift/README.md#L773-L789) · [Appearance sample](../swift/README.md#L318-L332)

## Remaining visual check

Start the branch app with `uv run python scripts/loopflow-dev.py run` for the pane demo. Use test settings and capture the same sample on the base and branch at the same display scale, font size and theme. Compare Loopflow Light and Loopflow Dark in a Task shell. Check a live Session for the same regular weight. Preview and cancel a color change while the sample is visible. Import a valid color theme containing `font-thicken = true` and check the omission message and regular strokes. Put the line in the test Ghostty config, reopen, and check that both panes show extra weight in either color theme. Remove the line and reopen to recover the default. If regular text still appears heavy with a confirmed `false`, use the real-pane comparison to investigate the selected face or rendering before calling the Task done.

```sh
printf 'Regular  Il1  0O  -> != =>\n\033[1mBold     Il1  0O\033[0m\n\033[3mItalic   Il1  0O\033[0m\n'
```
