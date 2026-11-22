# 50f7f9: review the bundled themes

The audit is done and the direction is chosen. What remains is writing the two default theme files, giving the title bar its own role, and checking both.

## Decided

- "i def prefer the new loopflow darks red, but i kinda wanna go back at least somewhat towards rhe bluer grey", then "you can pick for now lets stay higher level."
- Picked on that basis:
  - **Loopflow Light:** the repair (`light_repair` in `scratch/theme-audit/proposals.py`), 21 values change.
  - **Loopflow Dark:** B2, `dark_pair(dark, light, cool=0.67)`: Light's burgundy as rose ink on a neutral blue-grey ground `#2b2d30`, 38 values change.
  - **Title bar:** a stated role, `loopflow-titlebar`. Made for review at kickoff; see below for what it replaced.

## User-visible outcome

A new person opening Loopflow in dark mode sees the same burgundy title bar (`#722f37`) as in light mode, over a blue-grey window with rose headings and links. Status chips, diff lines, file selection and ANSI colors in both defaults are readable where the audit found them failing. The other 22 themes look as they do today, except five light title bars that get darker so their secondary text can be read.

## The title bar becomes a role

The earlier plan: stop `LoopflowPalette.titlebarBackground` (`swift/Loopflow/Design/BrandColors.swift:59`) mixing 65% of the ground into the accent in dark themes, or lower the mix if any other dark theme's bar fails. Measured with `audit.py`'s functions:

| mix | worst secondary text (72% white) among the other 11 dark themes | Loopflow Dark's bar |
|---|---|---|
| 0 (removed) | 3.34:1 Rosewood; all 11 are 3.34–3.39 | `#8a3a44`, chroma 0.109 |
| 0.30 | 4.05:1 Moss | `#6e363e`, 0.080 |
| 0.50 | 4.63:1 Moss | `#5a343a`, 0.055 |
| 0.65 (today) | 5.14:1 Moss | `#4c3237`, 0.038 |

Primary text passes everywhere (4.80+), because those 11 accents all sit at OKLCH L 0.55. Secondary text does not until the mix is about 0.5, and at 0.5 Loopflow Dark's bar has barely moved from today's (Light's burgundy is chroma 0.095). No one number gives Dark a burgundy bar and keeps the other 11 readable. Removing the mix would also turn 11 dark title bars into mid-grey bands at 3–4:1 against their windows, a redesign of themes this Task leaves alone.

So the bar is stated per theme, like every other color:

- `PaletteDefinition` gains `titlebar`; `ThemeFile`'s key table gains `("titlebar", \.titlebar)`; `ThemeEditor` gains a "Title bar" row under "Actions and focus".
- One rule fills a missing bar, `GhosttyThemeMapping.titlebar(accent:background:dark:)`: today's color (accent in light, accent mixed 65% into the ground in dark), then darkened toward black, same hue, until the 72% text reaches 4.5:1 (toward white when the bar's text is black). `palette(_:)` uses it for its own result. `ThemeFile.parse` calls it again **after** the file's stated roles and `loopflow-appearance` are applied, when the file has no `loopflow-titlebar`: a theme saved by the editor before this change states its accent, and its bar must come from that accent, not from ANSI 4.
- The `isDark ?` branch leaves `LoopflowPalette`; `titlebarBackground` reads the role, and `titlebarColorScheme` picks white or black against the bar, not against the accent. The 0.72 is named once and read by `titlebarSecondaryText`, the rule and the check.
- `PaletteDefinition.readabilityErrors` checks title bar text and its 72% level on the bar at 4.5:1. That is the gate a built-in passes to load.
- Catalogue: Loopflow Dark states `#722f37`, Light's own bar (white 9.65:1, secondary 5.85:1, 1.4:1 against `#2b2d30`: it separates by hue, as Light's does by value). It is the one theme the rule does not serve (the rule gives `#4c3237`). The other 23 state what the rule gives: for 18 that is the color they paint today, for the five below a darker one.
- Passed over: `#8a3a44` (Dark's button fill) as the bar; secondary text is 4.78:1, and the two defaults would no longer share one bar.
- Passed over: "today's bar exactly" as the fallback. Under it the derived fixture `747474/FFFFFF` gets bar `#717478`, secondary text 3.29:1, and step 2's own test fails; so does every light import whose accent lands at the mapping's 4.5:1 floor.

The same rule answers step 3's title bar failures without touching buttons, and moves no other built-in (measured with `audit.py`'s functions, 1/255 steps toward black, threshold 4.52):

| theme | bar today | secondary | rule gives | secondary |
|---|---|---|---|---|
| Catppuccin Latte | `#7d705c` | 3.37 | `#635849` | 4.52 |
| Gruvbox Light | `#836f58` | 3.35 | `#675745` | 4.52 |
| Flexoki Light | `#847032` | 3.36 | `#685827` | 4.53 |
| Linen | `#4c665b` | 4.15 | `#465e54` | 4.57 |
| Noctis Lilac | `#5a50a8` | 4.35 | `#574da3` | 4.52 |

Their accents and buttons stay. The files state what the Swift rule produces; if it differs from this table by a rounding step, the Swift value wins.

## Build

All five steps are built. What each one settled:

1. **The two defaults.** `scratch/theme-audit/write_defaults.py` writes both files from `proposals.py`, reading the base commit's files so a rerun writes the same bytes. The Swift suites passed on the first run: no derived ink (`linkInk`, `actionInk`, `selectionEdge`) needed a value moved.
2. **`loopflow-titlebar`** in all 24 files, the rule in `GhosttyThemeMapping.titlebar`, the check in `readabilityErrors`, the editor row. The Swift rule and `audit.py`'s port agree on every value in the table above.
3. **Changed words.** Body text moved until code on a changed word reads: Catppuccin Latte `#4f5054` → `#48494d`, Solarized Dark `#9b9e9e` → `#b6b9b9`. `emphasis-ink` moved with it (`ThemeTests` holds headings to the text color in the 22). Solarized Dark's app text is now visibly lighter than its terminal text (`#839496`); see `scratch/questions.md`.
4. **Words and prototype.** `desktop-themes.js` restates the catalogue again and carries `titlebar`; `.titlebar` and the Choose Theme sample bar paint `var(--titlebar)`. The `swift/README.md` example had comments after values, which `ThemeFile` rejects; they are on their own lines now.
5. **Page rebuilt.** The 24 sections read the files as they are; "Before" under the two defaults is the base commit.

Nothing remains to build. Left for ship-review: the editor's Title bar row and the bar in a key window (needs an unlocked display).

## Docs

`swift/README.md` (Appearance) and `swift/Loopflow/Themes/README.md` describe the new Loopflow Dark, `loopflow-titlebar`, its fallback and the editor's **Title bar** row. The Themes README example is Flexoki Light's real pair (`#847032`, `#685827`), matching the file.

## Acceptance check at gate

`swift test --package-path swift --filter 'WorkPaletteTests|TerminalColorTests|ThemeTests'` passes, including the new title bar expectation across 24 built-ins. Rebuilt page: Loopflow Light and Loopflow Dark list no text pair under 4.5:1 and no title bar finding in any theme. `scratch/concept-review.html` holds the model review and its four questions.

## Demo

Dev app, system in dark mode, fresh settings: burgundy title bar over blue-grey, rose headings. Settings → Choose Theme → Loopflow Light: the same bar. A display is needed, so this may fall to ship-review. Without one: `design/prototypes/desktop.html`, Theme menu, the two defaults side by side.

## Absent and error states

A personal theme file without `loopflow-titlebar` gets its bar from its own accent, ground and appearance: today's color where the second text level reads, a darker one where it does not. A saved custom theme from before this change decodes the same way. Once the editor saves a theme the bar is stated, and changing "Button" no longer moves it, as with hover and pressed.

## What goes

- The `isDark ? accent.mixed(...)` branch in `LoopflowPalette.titlebarBackground`.
- `dark_repair`'s standalone use and the A/B1/B3 variants on the page can stay as the record of the choice; nothing in `scratch/` ships.
- Stays: theme names, order and count (`ThemeTests` pins them), every theme's accent and buttons, user theme files.

## Checks

- `swift test --package-path swift --filter 'WorkPaletteTests|TerminalColorTests|ThemeTests'` (needs `cargo build -p loopflow --bin lf` first): 27 tests in 3 suites passed, 2026-11-22. Gate reruns.
- `python3 -I scratch/theme-audit/audit.py`: no fail in Loopflow Light or Loopflow Dark, no title bar finding in any theme.
- `cargo test -p loopflow --test integration documented_commands::` (README edits): not run; gate.
- Dev app in both defaults and two files without `loopflow-titlebar`, looked at on a private Machine with the screen locked: passed, 2026-11-22 (`scratch/qa.html`). Packaging failed first on a stale `ThirdPartyNotices/inventory.json`; refreshed. Open for ship-review: the editor's Title bar row and a key window's bar on an unlocked display.

## Left outside this change (for ship-review)

Catalogue-wide findings that are one rule in `GhosttyThemeMapping` repeated across 22 files, not unreadable roles:
- added and removed diff tints alike in all 24 (the Diff reuses the chip fill);
- file selection fill invisible in all 24 (`selection-fill` is `surface`);
- keyboard focus under 3:1 on the selected row in 21;
- secondary and tertiary text one color in 5;
- ANSI under 3:1 in 10;
- whether 24 is the right number (the page argues 16);
- what to take from Rex (generate roles instead of freezing them, a cursor in the accent color);
- separate diff roles (`diff-added`, `diff-removed`), so a chip fill and a diff line stop sharing one color;
- generating `design/prototypes/desktop-themes.js` from `Catalogue/` instead of restating 24 themes by hand.

## Rex direction (steer 54d66633)

"Also study the themes Rex (Superlogical) ships and say what Loopflow can learn from them: how many it ships and which are the defaults, which roles a Rex theme defines beyond the terminal palette, how its light and dark defaults relate, and how its interface and terminal are kept as one theme. Rex may not be installed on this Mac; check /Applications and ~/.config/rex first, then its public docs and site. Put the comparison on the same scratch page. Inspiration only: take ideas, never copy theme files, color values or code."

Done: it is the "What Rex does" section of the page, read from the installed Rex Beta bundle.

## The page

`scratch/theme-audit/index.html`, served on the tailnet at https://jacks-macbook-pro.tail0eda02.ts.net:8757/ by a python http.server (pid 5543, 127.0.0.1:8757) behind `tailscale serve --https=8757`. Stop with `kill 5543` and `tailscale serve --https=8757 off`.
