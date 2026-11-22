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

- `PaletteDefinition` gains `titlebar`; `ThemeFile`'s key table gains `("titlebar", \.titlebar)`; `ThemeEditor` gains a "Title bar" row.
- `GhosttyThemeMapping` derives it for theme files that omit it, with today's rule (accent in light, accent mixed 65% into the ground in dark). The `isDark ?` branch leaves `LoopflowPalette`; `titlebarBackground` reads the role, and `titlebarColorScheme` picks white or black against the bar, not against the accent.
- Catalogue: Loopflow Dark states `#722f37`, Light's own bar (white 9.65:1, secondary 5.85:1, 1.4:1 against `#2b2d30`: it separates by hue, as Light's does by value). The other 11 dark files state the value they compute today, so nothing moves. Light files state their accent, except the five below.
- Passed over: `#8a3a44` (Dark's button fill) as the bar; secondary text is 4.78:1, and the two defaults would no longer share one bar.

This also answers step 3's title bar failures without touching buttons. Secondary text on a light theme's accent bar: Catppuccin Latte 3.37, Gruvbox Light 3.35, Flexoki Light 3.36, Linen 4.15, Noctis Lilac 4.35. Each states a bar darkened from its accent, same hue, until 72% white reaches 4.5:1 (white on the bar about 6.6:1). Their accents and buttons stay.

## Build

Slices, each run before the next.

1. **The two defaults, tested.** Write `changes(before, after)` for both into `swift/Loopflow/Themes/Catalogue/Loopflow Light` and `Loopflow Dark` (every role and ANSI 0–15 stays stated), then `swift test --package-path swift --filter 'WorkPaletteTests|TerminalColorTests|ThemeTests'`. This is the slice that can prove the plan wrong: the proposals were settled by `audit.py`, and the Swift suites also check inks `audit.py` has no name for (`linkInk`, `linkHover`, `actionInk`, `selectionEdge`, terminal `readabilityErrors`). If a derived ink fails, adjust the value in `proposals.py` and regenerate; the files are never hand-tuned away from the script.
2. **`loopflow-titlebar`** as above, in all 24 files, with a `WorkPaletteTests` expectation that title bar text and its 72% secondary reach 4.5:1 on the bar in every built-in and in the five derived fixtures. No test reads the title bar today. `audit.py`'s `derived()` reads the role so the page agrees with the app.
3. **Unreadable roles elsewhere:** code on a changed word in Catppuccin Latte and Solarized Dark diffs. Nothing else in the other 22.
4. **Words and prototype.** `swift/Loopflow/Themes/README.md` ("the original slate with bronze") and `swift/README.md:308` ("slate with warm bronze") describe the old Dark; both gain the title bar role where they list roles. `design/prototypes/desktop-themes.js` carries every theme's values by hand and gets the new ones plus `titlebar`; `desktop.css:83` paints `.titlebar` with `var(--accent)` and moves to the new variable (the prototype never had the dark mix, so its dark bars are wrong today).
5. `python3 -I scratch/theme-audit/build.py`, so "Now" on the page shows what shipped.

## Acceptance check at gate

`swift test --package-path swift --filter 'WorkPaletteTests|TerminalColorTests|ThemeTests'` passes, including the new title bar expectation across 24 built-ins. Rebuilt page: Loopflow Light and Loopflow Dark list no text pair under 4.5:1 and no title bar finding in any theme.

## Demo

Dev app, system in dark mode, fresh settings: burgundy title bar over blue-grey, rose headings. Settings → Choose Theme → Loopflow Light: the same bar. A display is needed, so this may fall to ship-review. Without one: `design/prototypes/desktop.html`, Theme menu, the two defaults side by side.

## Absent and error states

A personal theme file without `loopflow-titlebar` gets today's bar exactly. A saved custom theme from before this change decodes the same way, through the mapping.

## What goes

- The `isDark ? accent.mixed(...)` branch in `LoopflowPalette.titlebarBackground`.
- `dark_repair`'s standalone use and the A/B1/B3 variants on the page can stay as the record of the choice; nothing in `scratch/` ships.
- Stays: theme names, order and count (`ThemeTests` pins them), every theme's accent and buttons, user theme files.

## Checks

- `WorkPaletteTests`, `TerminalColorTests`, `ThemeTests`: not run yet.
- Dev app in both defaults, looked at: not run yet; needs a display, ship-review if not here.

## Left outside this change (for ship-review)

Catalogue-wide findings that are one rule in `GhosttyThemeMapping` repeated across 22 files, not unreadable roles:
- added and removed diff tints alike in all 24 (the Diff reuses the chip fill);
- file selection fill invisible in all 24 (`selection-fill` is `surface`);
- keyboard focus under 3:1 on the selected row in 21;
- secondary and tertiary text one color in 5;
- ANSI under 3:1 in 10;
- whether 24 is the right number (the page argues 16);
- what to take from Rex (generate roles instead of freezing them, a cursor in the accent color).

## Rex direction (steer 54d66633)

"Also study the themes Rex (Superlogical) ships and say what Loopflow can learn from them: how many it ships and which are the defaults, which roles a Rex theme defines beyond the terminal palette, how its light and dark defaults relate, and how its interface and terminal are kept as one theme. Rex may not be installed on this Mac; check /Applications and ~/.config/rex first, then its public docs and site. Put the comparison on the same scratch page. Inspiration only: take ideas, never copy theme files, color values or code."

Done: it is the "What Rex does" section of the page, read from the installed Rex Beta bundle.

## The page

`scratch/theme-audit/index.html`, served on the tailnet at https://jacks-macbook-pro.tail0eda02.ts.net:8757/ by a python http.server (pid 5543, 127.0.0.1:8757) behind `tailscale serve --https=8757`. Stop with `kill 5543` and `tailscale serve --https=8757 off`.
