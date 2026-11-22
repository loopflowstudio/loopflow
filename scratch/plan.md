# 50f7f9: review the bundled themes

Built, in PR #2020: the two defaults and the `loopflow-titlebar` role (round 1), then the 13 families and a Choose Theme grid of cards (round 2). What remains is ship-review's look at the Dev app on an unlocked display.

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
- Not run here: `test_third_party.py` (gate); Dev app chooser, editor Title bar row, a key window's bar (ship-review, unlocked display).

## Demo

Dev app → Settings → Choose Theme: 26 cards in their own colors, pairs side by side. Click Fig and the window turns plum with lime; Fig Leaf is the light half. A settings file naming Gruvbox Dark opens in Loopflow Dark, and a sentence says why.

## Remaining

- Ship-review: the Dev app look above.

## Left outside this Task

Import mapping rules (diff tints, file selection fill, focus on the selected row), separate `diff-added`/`diff-removed` roles, picking a family once in System mode, and Rex's accent-colored cursor for imports. The generated themes already follow these rules; imported themes don't yet.

## The page

`scratch/theme-audit/index.html` is served at https://jacks-macbook-pro.tail0eda02.ts.net:8757/ by a python http.server (pid 5543, 127.0.0.1:8757) behind `tailscale serve --https=8757`. Stop it with `kill 5543` and `tailscale serve --https=8757 off`.
