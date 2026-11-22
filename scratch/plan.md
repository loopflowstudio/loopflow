# 50f7f9: review the bundled themes

Built, in PR #2020: the two defaults and the `loopflow-titlebar` role (round 1), then the 13 families and a Choose Theme grid of cards (round 2). What remains is three chooser corrections from the concept review, then ship-review's look at the Dev app on an unlocked display.

## What people said

- Round 1: "i def prefer the new loopflow darks red, but i kinda wanna go back at least somewhat towards rhe bluer grey", then "you can pick for now lets stay higher level."
- Round 2, at ship-review after `scratch/theme-audit/explore.html`: "i like the 13 damilies"; of the page's cards, "our theme chooser should be more like this so you cab preview and scan"; then "jusr put all this stifg in this one task".
- Rex steer (54d66633): "Inspiration only: take ideas, never copy theme files, color values or code." Answered in the "What Rex does" section of `scratch/theme-audit/index.html`.

## What exists

- **Defaults.** Loopflow Light is the audit repair (21 values). Loopflow Dark is Light's burgundy as rose ink on blue-grey `#2b2d30` (`dark_pair(..., cool=0.67)` in `scratch/theme-audit/proposals.py`). Both state title bar `#722f37`. The generator reads these two files and never rewrites them.
- **Title bar role.** `loopflow-titlebar` in `PaletteDefinition`/`ThemeFile`/editor ("Title bar" row). Missing bar: `GhosttyThemeMapping.titlebar` takes the old color (accent; in dark, 65% ground mixed in) and darkens it (or lightens it, under black text) until the 72% text reaches 4.5:1. It is applied after a file's stated roles. `readabilityErrors` checks both text levels on the bar. Why a role and not a new mix: no single mix gave Dark a burgundy bar and kept the other dark bars' secondary text at 4.5:1 (table in git history, `8ae51f65b9:scratch/plan.md`).
- **Catalogue.** 13 families, 13 light and 13 dark. `scripts/theme_catalogue.py` writes the 24 generated files and `design/prototypes/desktop-themes.js`; `--check` lists drift (passes at b92c506fe4), run by `python/tests/test_theme_catalogue.py`. Sorting by name after the Loopflow pair keeps each family's pair together. The companions, `Sources.md` and `Licenses/` are gone.
- **Retired names.** `ThemeDefinition.retired` lists the 22 with their side. `AppearanceStore.selection(from:)` maps each one to Loopflow Light or Dark and puts a sentence in the notice. The settings file keeps the old name until Apply.
- **Chooser.** `ThemeCard` grid in `ThemeChooser.swift`, each card drawn in its own roles. The chosen card gets a 2 pt ring in `keyboardFocus`. `ThemeSample` is deleted.

## Checks

- `swift test --package-path swift --filter 'WorkPaletteTests|TerminalColorTests|ThemeTests|ThemeWorkflowTests'` (build `lf` first): 32 passed, 2026-11-22.
- `uv run pytest python/tests/test_theme_catalogue.py`: passed. `documented_commands::`: 4 passed.
- All 26 cards rendered offscreen with `ImageRenderer`: `scratch/qa/cards.png`.
- Round 1 Dev app QA, both defaults plus two files without a bar, with the screen locked: passed (`scratch/qa.html`).
- Round 2 Dev app QA, screen locked: retired-name notice, both sides' cards, Fig preview and Fig applied (via `lf set`): passed after top-aligning the grid rows and refreshing the third-party inventory (`scratch/qa.html`).
- `test_third_party.py`: 9 passed.
- Not run here: Apply clicked in the chooser, the editor's Title bar row, a key window's bar, the selected Appearance toolbar tab in dark mode (ship-review, unlocked display).

## Demo

Dev app → Settings → Choose Theme: 13 cards per side in their own colors, both sides in System mode. Click Fig and the window turns plum with lime; Fig Leaf is the light half. A settings file naming Gruvbox Dark opens in Loopflow Dark, and a sentence says why.

## Remaining

From the concept review of round 2 (`scratch/concept-review.html`). The chooser is now sections of cards, one per side; three pieces of the list-per-side chooser it replaced still stand.

1. **Delete the "Light theme" / "Dark theme" buttons** in `ThemeChooser.body` (`appearance-picker-<side>`). In System mode both sections are on screen and the chosen card in each is ringed, so the buttons only repeat the ring and switch `picker`, which a card click already does. Each section heading carries the chosen name instead: "Light themes · Fjord Light" (in Light or Dark mode, the one section's heading). `picker` stays in `AppearanceStore` as the side of the last card clicked; `showPicker` stays as the store call a card makes. `ThemeWorkflowTests` stops finding the buttons and checks the headings and that a card click moves the Customize row.
2. **Import files a theme on its own side; no refusal.** `AppearanceStore.importTheme` reads the file and uses `file.theme.isDark` for the side: it stages the file, assigns it to that side, sets `picker` to it, and in Light or Dark mode switches the preview's mode to the file's side (the segmented control shows it; Cancel undoes it). Delete `AppearanceDraft.refusal`, `AppearanceStore.refusal`, the `refusal == nil` in `canApply`, the `appearance-import-refusal` text, and `importPlaceholder`'s `side` and `showPicker` call. The Import tile stays in each Yours grid. Tests: the two refusal tests (`ThemeTests` "Personal", `ThemeWorkflowTests.chooserExplainsAMismatchedImportAndKeepsApplyDisabled`) become "a dark file imported from the light section previews in Dark and Apply is enabled".
3. **The retired-name sentence says only what is true.** Today it says "to keep it, import its Ghostty theme file under Yours"; Linen and Hearth never had one, Clay's was named Desert Morning, and no upstream file carries the app roles the built-in had. New sentence: "“Gruvbox Dark” is no longer built in, so Loopflow Dark is showing." Update `ThemeTests` line ~258 if its prefix changes (it does not), and the last sentence of `swift/Loopflow/Themes/README.md` ("and says how to import the old one" → "and says so").

Checks for these: the Swift filter under Checks; no theme file changes, so the catalogue check is untouched.

- Ship-review: the Dev app look above, on an unlocked display, including a dark file imported from the light section.

## Left outside this Task

Import mapping rules (diff tints, file selection fill, focus on the selected row), separate `diff-added`/`diff-removed` roles, picking a family once in System mode (it needs a family the app can read; today a family is a name prefix only the generator knows), and Rex's accent-colored cursor for imports. The generated themes already follow these rules; imported themes don't yet.

## The page

`scratch/theme-audit/index.html` is served at https://jacks-macbook-pro.tail0eda02.ts.net:8757/ by a python http.server (pid 5543, 127.0.0.1:8757) behind `tailscale serve --https=8757`. Stop it with `kill 5543` and `tailscale serve --https=8757 off`.
