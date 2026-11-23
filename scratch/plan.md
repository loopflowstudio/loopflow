# 50f7f9: review the bundled themes

Built, in PR #2020: the two defaults and the `loopflow-titlebar` role (round 1), then the 13 families and a Choose Theme grid of cards (round 2). Round 3 made the concept review's three chooser corrections, swapped Verdigris for Moss and dropped the removed-name fallback. Round 3's concept review found two leftovers of the per-side picker; round 4 removed them: `previewTheme` and `importTheme` share one assignment that takes the side from the theme's own colors, `showPicker` is gone and `picker` is `previewSide`. Round 4 QA passed. Concept review round 4 shaped the steer's iPhone and website pieces; implement built them and round 5 QA passed them, with one open Desktop glitch. Concept review round 5 sent one change back to implement (the notice split); round 6 built it, and round 6 QA passed it after making a failed read retry. Concept review round 6 found nothing to change (`scratch/concept-review.html`). The demo (`scratch/demo.md`) asked for three changes; implement round 7 built them: Harbor dropped (12 families, 24 themes), Arcade Day on the Paper ground, Customize and Delete in the bottom bar. Round 7 QA passed them after replacing the chooser's `LazyVGrid`, which drew blank or overlapping grids after Settings was reopened. Concept review round 7 found the bar's buttons don't say which theme they act on in System mode (`scratch/concept-review.html`). Next: implement names it, then ship-review.

## What people said

- Round 1: "i def prefer the new loopflow darks red, but i kinda wanna go back at least somewhat towards rhe bluer grey", then "you can pick for now lets stay higher level."
- Round 2, at ship-review after `scratch/theme-audit/explore.html`: "i like the 13 damilies"; of the page's cards, "our theme chooser should be more like this so you cab preview and scan"; then "jusr put all this stifg in this one task".
- Steers on round 3: "can we swap verdigris for somethig more moss like"; on removed names, "not worried about any existing users".
- Steer f4ce64ed, during round 4 QA: "make sure to make ipgone app respecy themes and the website he in loopflow light".
- Demo, 2026-11-22: "Arcade day's background is too bubblegum"; of Cream and Paper, "i tink i prefer Paper"; "i also dont like how the Customize button makes the rows jump"; "let's drop harbor also", "to get down to 12".
- Rex steer (54d66633): "Inspiration only: take ideas, never copy theme files, color values or code." Answered in the "What Rex does" section of `scratch/theme-audit/index.html`.

## What exists

- **Defaults.** Loopflow Light is the audit repair (21 values). Loopflow Dark is Light's burgundy as rose ink on blue-grey `#2b2d30` (`dark_pair(..., cool=0.67)` in `scratch/theme-audit/proposals.py`). Both state title bar `#722f37`. The generator reads these two files and never rewrites them.
- **Title bar role.** `loopflow-titlebar` in `PaletteDefinition`/`ThemeFile`/editor ("Title bar" row). Missing bar: `GhosttyThemeMapping.titlebar` takes the old color (accent; in dark, 65% ground mixed in) and darkens it (or lightens it, under black text) until the 72% text reaches 4.5:1. It is applied after a file's stated roles. `readabilityErrors` checks both text levels on the bar. Why a role and not a new mix: no single mix gave Dark a burgundy bar and kept the other dark bars' secondary text at 4.5:1 (table in git history, `8ae51f65b9:scratch/plan.md`).
- **Catalogue.** 12 families, 12 light and 12 dark (Harbor dropped at the demo; Arcade Day's ground is Paper, oklch 0.975 0.006 300, `#f7f6fa`). `scripts/theme_catalogue.py` writes the 22 generated files and `design/prototypes/desktop-themes.js`; `--check` lists drift, run by `python/tests/test_theme_catalogue.py`. Its `titlebar()` mirrors `GhosttyThemeMapping.titlebar`, so a spec's `titlebar="accent"` is the bar the app would derive (compress moved Signal Light, Tannin Light and Fjord Light by one or two steps). Sorting by name after the Loopflow pair keeps each family's pair together. The companions, `Sources.md` and `Licenses/` are gone.
- **Removed names.** A setting naming a pre-family built-in (Gruvbox Dark, Linen…) is an unknown name, as on main: "No theme named …: keeping …".
- **Chooser.** `ThemeCard` grid in `ThemeChooser.swift`, each card drawn in its own roles. The chosen card gets a 2 pt ring in `keyboardFocus`. Each section heading names its chosen theme ("Light themes · Fjord Light"); the Light/Dark theme buttons are gone. Customize and Delete/Revert sit in the bottom bar beside Cancel/Apply and act on the chosen theme, so choosing a card never changes a row's height. The grids are `CardGrid`, a non-lazy `Layout` (equal columns ≥ 180 pt, rows as tall as their tallest card). `importTheme` files a theme on the side its colors say and, in Light or Dark mode, switches the preview mode; there is no refusal.
- **One Machine fact.** `theme:` in the serving Machine's `~/.lf/config.yaml`, read through `config.theme {value}` (no `path`); `ThemeChoice` (`swift/Loopflow/Design/ThemeChoice.swift`) is the one parser. Desktop takes the config path from the answer (`AppearanceStore.configPath`). The phone's `MachineTheme` (`swift/LoopflowiOS/MachineTheme.swift`) reads it on connect and foreground, caches per Machine (`theme-<machineID>`), and `MachineConnectionView` sets `\.theme`, `\.palette`, `\.colorScheme`. `scripts/theme_catalogue.py` writes `website/static/theme.css` from Loopflow Light.
- **Read failures.** `AppearanceStore.readFailure` holds a failed `config.theme` read or write, retries every second, and shows in the banner without Choose Theme…; `ServerLink` turns a parse refusal into "The server (Y) can't read what this Loopflow (X) asked. Update the older one."
- **Moss** replaces Verdigris (spec in `scripts/theme_catalogue.py`; both pass the audit).

## Checks

- `swift test --package-path swift --filter 'WorkPaletteTests|TerminalColorTests|ThemeTests|ThemeWorkflowTests'` (build `lf` first): 31 passed, 2026-11-22 (round 3, after Moss and the removed-name deletion).
- `uv run pytest python/tests/test_theme_catalogue.py`: passed. `documented_commands::`: 4 passed.
- All 26 cards rendered offscreen with `ImageRenderer`: `scratch/qa/cards.png`.
- Round 1 Dev app QA, both defaults plus two files without a bar, with the screen locked: passed (`scratch/qa.html`).
- Round 2 Dev app QA, screen locked: both sides' cards, Fig preview and Fig applied (via `lf set`): passed after top-aligning the grid rows and refreshing the third-party inventory (`scratch/qa.html`).
- Round 3 Dev app QA, screen locked: Verdigris as an unknown name, headings without side buttons, a dark file imported in Light mode (switches to Dark) and System mode (stays), Moss applied via `lf set`: passed after dropping the empty "Derived: ." import line (`scratch/qa.html`).
- `swift test … 'ThemeTests|ThemeWorkflowTests|WorkPaletteTests|TerminalColorTests'`: 32 passed, 2026-11-22 (after the Derived fix).
- `test_third_party.py`: 9 passed.
- `swift test … 'WorkPaletteTests|TerminalColorTests|ThemeTests|ThemeWorkflowTests'`: 33 passed, 2026-11-22 (round 4, with `previewingADarkThemeFromTheLightSideLandsOnTheDarkSide`).
- `swift test … 'WorkPaletteTests|TerminalColorTests|ThemeTests|ThemeWorkflowTests'`: 33 passed and `test_theme_catalogue.py` passed, 2026-11-22 (compress: one title bar rule, `show(_:)`).
- Round 4 Dev app QA, screen locked: a light card in Light mode, a dark card in System mode, a dark import in Light mode: passed after dropping themes from the third-party inputs (`scratch/qa.html`). `test_third_party.py`: 9 passed.
- Not run here: the real Import file panel, Apply and Cancel clicked in the chooser, the editor's Title bar row, a key window's bar, the selected Appearance toolbar tab in dark mode (ship-review, unlocked display).

- 2026-11-22, implement round 5: `cargo test -p loopflow --lib -- lf::commands::write::tests:: server::schema::tests::` 11 passed (after regenerating `server_schema.json`); `served_reads configuration_edits_use_the_server_without_work_processes` passed; `swift test … 'WorkPaletteTests|TerminalColorTests|ThemeTests|ThemeWorkflowTests|AppAppearanceTests|ServerProtocolTests|ServerClientTests'` 79 passed; `scripts/test_ios.py` `MachineThemeTests` 3 passed and `MobileTaskCreationTests` 21 passed; website `dev.py test` 99 passed; `test_theme_catalogue.py` passed; `documented_commands::` 4 passed. Not run: clippy (gate).

- 2026-11-22, compress round 5: `swift test … 'ThemeTests|ThemeWorkflowTests|AppAppearanceTests|ServerProtocolTests'` 53 passed; `test_ios.py` `MachineThemeTests` 3 passed; `theme_catalogue.py --check` and `test_theme_catalogue.py` passed.

- 2026-11-22, QA round 5 (`scratch/qa.html`): site crawl, 73 pages, no contrast or Pico-blue failures after fixing the Company focus ring; phone on a simulator paired with a private branch Machine: Fig Leaf/Fig by appearance, single name, unknown name, no name, Machine away, sheets; Desktop on the private Machine, screen locked: launch read, live edit, served write, unknown-name path. Open: light cards missing from Choose Theme while the unknown-name notice is up (R5.10); version skew both ways (R5.7, R5.9).

- 2026-11-22, implement round 6: `swift test … 'AppAppearanceTests|ServerClientTests'` 25 passed, including `failedRead` and both cases of `aRequestTheServerCannotParseNamesBothBuilds`. Not run: the Dev app against the live 0.16.0 server (ship-review), clippy (gate).

- 2026-11-22, compress round 6: the parse sentence folded into `ServerLink.answered` (no public `unparsed`). `swift test … 'AppAppearanceTests|aRequestTheServerCannotParseNamesBothBuilds'` 5 and 2 cases passed.

- 2026-11-22, QA round 6 (`scratch/qa.html`), screen locked: Dev app against a 0.16.0 server shows the both-builds sentence with no button and Choose Theme still draws its cards; swapping to the branch server left the banner stuck until relaunch, fixed by retrying while `readFailure` is set (rerun: cleared in 12 s, Fig Leaf applied). Unknown-name notice keeps Choose Theme…; R5.10 did not reproduce. `swift test … 'AppAppearanceTests|ThemeWorkflowTests'` 8 passed.

- 2026-11-22, implement round 7: `swift test … 'ThemeTests|ThemeWorkflowTests'` 27 passed (before the bottom-bar move), then `ThemeWorkflowTests` 4 passed after it; `theme_catalogue.py --check` and `test_theme_catalogue.py` passed. The runtime 13/13 count precondition in `ThemeDefinition.builtins` is deleted; `catalogueIsTwelveFamiliesWithALightAndADarkTheme` names the list. Not run: `documented_commands::` for the `swift/README.md` edit (gate).

- 2026-11-22, compress round 7: one `editingYours` in `ThemeChooser`, catalogue docstrings without counts. `ThemeWorkflowTests` 4 passed; `theme_catalogue.py --check` passed.

- 2026-11-22, QA round 7 (`scratch/qa.html`), Dev app behind a display in use: 12 + 12 cards, no Harbor; choosing a card leaves every row edge in place; Delete/Revert in the bar, staged until Apply; Cancel restores; editor opens on one Customize click; Apply wrote `light:Arcade Day,dark:Loopflow Dark`; a Harbor setting reads as an unknown name; the bar fits at 700 pt. Fixed: blank or overlapping grids after reopening Settings (R5.10), by `CardGrid`. `swift test … 'ThemeWorkflowTests|ThemeTests'` 27 passed.

## Demo

Dev app → Settings → Choose Theme: 12 cards per side in their own colors, both sides in System mode. Click Fig and the window turns plum with lime; Fig Leaf is the light half. Import a dark file from the light section in Light mode: the mode switches to Dark and the file previews there.

## Remaining

**Implement: the bar names its target.** In System mode two cards are ringed (one per side) but Customize and Delete/Revert act on `selectedTheme`, the side clicked last; before round 7 their place under the card said which, now nothing does. The buttons carry the name: "Customize \(selectedTheme.name)", "Delete …", "Revert … to built-in" (`swift/LoopflowMac/Views/ThemeChooser.swift`, the bottom `HStack`). Keep "Previewing X" (it names the window's colors while the editor is open). In `ThemeWorkflowTests`, replace `customizeShown(under:)`, which can no longer fail, with: after tapping Fjord Light then Signal in System mode, the `appearance-customize-theme` button reads "Customize Signal"; tapping Fjord Light again makes it "Customize Fjord Light". Check: `swift test --package-path swift --filter ThemeWorkflowTests`, and the bar still fits at 700 pt with the longest name (QA's `r7-narrow` case). Passed over: a second ring style for the previewed card, or a side picker in the bar.

**Ship-review.** The Dev app on an unlocked display: the Import panel, the editor's Title bar row, a key window's bar, a dark import from the light section, and a real close and reopen of Settings with the grids drawn. Arcade Day on Paper, against the demo's "too bubblegum". The phone in Fig on a simulator against a Machine whose `theme:` is `light:Fig Leaf,dark:Fig`, and the site (`scratch/qa/home.png`, `scratch/qa/docs.png` are headless renders).

## Left outside this Task

The phone showing your own theme files or Ghostty themes from the Machine (the Machine would serve theme text; Rust would have to repeat Desktop's resolution order). Import mapping rules (diff tints, file selection fill, focus on the selected row), separate `diff-added`/`diff-removed` roles, picking a family once in System mode (it needs a family the app can read; today a family is a name prefix only the generator knows), and Rex's accent-colored cursor for imports. The generated themes already follow these rules; imported themes don't yet.

## The page

`scratch/theme-audit/index.html` is served at https://jacks-macbook-pro.tail0eda02.ts.net:8757/ by a python http.server (pid 5543, 127.0.0.1:8757) behind `tailscale serve --https=8757`. Stop it with `kill 5543` and `tailscale serve --https=8757 off`.
