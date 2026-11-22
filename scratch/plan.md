# 50f7f9: review the bundled themes

Built, in PR #2020: the two defaults and the `loopflow-titlebar` role (round 1), then the 13 families and a Choose Theme grid of cards (round 2). Round 3 made the concept review's three chooser corrections, swapped Verdigris for Moss and dropped the removed-name fallback. Round 3's concept review found two leftovers of the per-side picker; round 4 removed them: `previewTheme` and `importTheme` share one assignment that takes the side from the theme's own colors, `showPicker` is gone and `picker` is `previewSide`. Round 4 QA passed. Concept review round 4 shaped the steer's iPhone and website pieces; implement built them and round 5 QA passed them, with one open Desktop glitch; then ship-review.

## What people said

- Round 1: "i def prefer the new loopflow darks red, but i kinda wanna go back at least somewhat towards rhe bluer grey", then "you can pick for now lets stay higher level."
- Round 2, at ship-review after `scratch/theme-audit/explore.html`: "i like the 13 damilies"; of the page's cards, "our theme chooser should be more like this so you cab preview and scan"; then "jusr put all this stifg in this one task".
- Steers on round 3: "can we swap verdigris for somethig more moss like"; on removed names, "not worried about any existing users".
- Steer f4ce64ed, during round 4 QA: "make sure to make ipgone app respecy themes and the website he in loopflow light".
- Rex steer (54d66633): "Inspiration only: take ideas, never copy theme files, color values or code." Answered in the "What Rex does" section of `scratch/theme-audit/index.html`.

## What exists

- **Defaults.** Loopflow Light is the audit repair (21 values). Loopflow Dark is Light's burgundy as rose ink on blue-grey `#2b2d30` (`dark_pair(..., cool=0.67)` in `scratch/theme-audit/proposals.py`). Both state title bar `#722f37`. The generator reads these two files and never rewrites them.
- **Title bar role.** `loopflow-titlebar` in `PaletteDefinition`/`ThemeFile`/editor ("Title bar" row). Missing bar: `GhosttyThemeMapping.titlebar` takes the old color (accent; in dark, 65% ground mixed in) and darkens it (or lightens it, under black text) until the 72% text reaches 4.5:1. It is applied after a file's stated roles. `readabilityErrors` checks both text levels on the bar. Why a role and not a new mix: no single mix gave Dark a burgundy bar and kept the other dark bars' secondary text at 4.5:1 (table in git history, `8ae51f65b9:scratch/plan.md`).
- **Catalogue.** 13 families, 13 light and 13 dark. `scripts/theme_catalogue.py` writes the 24 generated files and `design/prototypes/desktop-themes.js`; `--check` lists drift, run by `python/tests/test_theme_catalogue.py`. Its `titlebar()` mirrors `GhosttyThemeMapping.titlebar`, so a spec's `titlebar="accent"` is the bar the app would derive (compress moved Signal Light, Tannin Light and Fjord Light by one or two steps). Sorting by name after the Loopflow pair keeps each family's pair together. The companions, `Sources.md` and `Licenses/` are gone.
- **Removed names.** A setting naming a pre-family built-in (Gruvbox Dark, Linen…) is an unknown name, as on main: "No theme named …: keeping …".
- **Chooser.** `ThemeCard` grid in `ThemeChooser.swift`, each card drawn in its own roles. The chosen card gets a 2 pt ring in `keyboardFocus`. Each section heading names its chosen theme ("Light themes · Fjord Light"); the Light/Dark theme buttons are gone. `importTheme` files a theme on the side its colors say and, in Light or Dark mode, switches the preview mode; there is no refusal.
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

## Demo

Dev app → Settings → Choose Theme: 13 cards per side in their own colors, both sides in System mode. Click Fig and the window turns plum with lime; Fig Leaf is the light half. Import a dark file from the light section in Light mode: the mode switches to Dark and the file previews there.

## Remaining

Concept review round 4 made the theme choice one Machine fact (`theme:` in the serving Machine's `~/.lf/config.yaml`), read through that Machine's server and resolved by one shared parser; the website consumes the Loopflow Light file. Implement built all three pieces:

- **One choice, one parser** (`6a57fba693`). `config.theme {value}` has no `path`; the server answers from its own config. `ThemeChoice` (`swift/Loopflow/Design/ThemeChoice.swift`) replaces `ThemeConfig`; `ThemePaths` lost `config`, and Desktop learns the config path from the server's answer (`AppearanceStore.configPath`: the unknown-name message, the file watch and Advanced use it). Theme fixtures put their files in the test server's `LF_HOME`.
- **The phone draws its Machine's theme** (`44d891ba7a`). `MachineTheme` (`swift/LoopflowiOS/MachineTheme.swift`) reads `config.theme` from `TaskListView` on connect and on return to the foreground, keeps each Machine's last answer in UserDefaults (`theme-<machineID>`), and `MachineConnectionView` sets `\.theme`, `\.palette` and `\.colorScheme` once. The views' `theme:` parameters and the three `colorScheme` computations are gone.
- **loopflow.studio is Loopflow Light** (`f7e16f2686`). `scripts/theme_catalogue.py` writes `website/static/theme.css`; `style.css` has no color of its own. The page background is `--background`, cards `--surface`; the light terminal blocks got a `--border` edge because Light's terminal background is the page background.

Next: **ship-review.** The Dev app on an unlocked display (Import panel, Apply/Cancel, the editor's Title bar row, a key window's bar, a dark import from the light section, Choose Theme with an unknown-name notice up: are the light cards there?), the phone in Fig on a simulator against a Machine whose `theme:` is `light:Fig Leaf,dark:Fig`, and the site (`scratch/qa/home.png`, `scratch/qa/docs.png` are headless renders).

## Left outside this Task

The phone showing your own theme files or Ghostty themes from the Machine (the Machine would serve theme text; Rust would have to repeat Desktop's resolution order). Import mapping rules (diff tints, file selection fill, focus on the selected row), separate `diff-added`/`diff-removed` roles, picking a family once in System mode (it needs a family the app can read; today a family is a name prefix only the generator knows), and Rex's accent-colored cursor for imports. The generated themes already follow these rules; imported themes don't yet.

## The page

`scratch/theme-audit/index.html` is served at https://jacks-macbook-pro.tail0eda02.ts.net:8757/ by a python http.server (pid 5543, 127.0.0.1:8757) behind `tailscale serve --https=8757`. Stop it with `kill 5543` and `tailscale serve --https=8757 off`.
