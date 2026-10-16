# LOO-488 — Whole-app theming

Status: **accepted for implementation through demo review**, October 15, 2026.
Jack Heart approved implementation, subsequently requested the palette redesign,
and authorized launching a separate preview. Native visual judgment remains open;
no landing, installation or replacement of running clients is implied.

**Base:** parent LOO-487 / [PR #1559](https://github.com/loopflowstudio/loopflow/pull/1559)
merged to main October 15 and is contained in this branch (main `4426b0b423`
merged at `11b5d18d5e`); PR #1560 now targets main directly. Its renderer/
navigation/pane contract is summarized in [terminal polish](terminal-polish.md).
The separate [Workflow-loading hold](workflow-loading.md) survives.

## Problem and live decisions

Jack wants a coherent workspace across navigation, headers, controls, file/diff
editors and terminal panes. Themes must preserve readable selection, focus, actions
and activity while switching retained Repo/Wave/Task contexts without losing
clients, drafts, focus ownership or layout.

Live October 15 decisions (the slot library, Edit + Reset and per-slot import that
preceded them are superseded; history at `3d05e621c4` and `b2a728f8eb`):

- Every theme controls every LoopflowPalette role. Ghostty RGB imports convert once
  into the full app contract. No hardcoded burgundy survives outside an authored preset.
- **Loopflow Light** keeps the original cream/burgundy design. **Loopflow Dark** is
  the old slate with burgundy buttons and warm bronze heading/focus ink (`#CEBFA9`)
  and Work selection (`#AB9B86`); exact values remain review judgment. The quieter
  pair is **Linen** and **Hearth**.
- Fixed **12 Light + 12 Dark** built-ins (listed below). Naming: keep an upstream
  name where reuse is clearly permitted, otherwise one-word material names; no
  product/trademark names.
- Licensing: exclude anything not 100% clearly licensed (each theme keeps its
  author's license; the iTerm2-Color-Schemes MIT covers the collection only).
  Night Owl and Noctis ship from their MIT upstream colors. Jack approved the
  license-clean catalogue in `scratch/theme-catalogue/index.html`.
- Mode is Light, Dark or System. One header row: Mode, then Light/Dark pickers.
  Each picker lists only its appearance, in System mode too; Light or Dark mode
  dims the other. Never one mixed list. Import into the wrong picker explains
  itself and Apply stays disabled.
- Liquid Glass is separate work Jack sees no reason to pursue now; no Task filed.

## Demo

Open Appearance → Choose Theme with the same conversation, shell and unsaved file
open. The Light picker lists 12 light built-ins, the Dark picker 12 dark ones.
Customize Linen and change a color: your copy overrides it. Delete the copy and
Linen returns. Import a Ghostty theme file into the matching picker; a dark file
in the Light picker explains itself and can't be applied. Apply System: `theme:`
in `~/.lf/config.yaml` reads `light:…,dark:…`. Apply Light: it reads one name.
Edit `theme:` by hand and the app follows; a misspelled name keeps the last theme
and names the problem. Cancel writes nothing. Relaunch restores the file's choice.

Switch Work, open temporary/pinned navigation, inspect Needs you, split/focus panes
and revisit unchanged diff text throughout. Clients, drafts, selections and layouts
remain. System changes choose Light/Dark assignments, not a palette's brightness.
Live provider output and visual comfort belong to review; truecolor output can
retain its own colors. No test substitutes for that judgment.

**Preview:** earlier `/tmp` previews predate theme files; a fresh preview needs
a rebuild at review.

## Chosen architecture and source of truth

`ThemeDefinition` is a name, complete `PaletteDefinition` and 256-color
`TerminalPalette`; the name is its identity. `LoopflowPalette` projects it.
`ThemeFile` is the one format and parser: Ghostty theme syntax plus optional
`loopflow-<role>` keys, `loopflow-appearance` and `loopflow-selection-opacity`.
Built-ins (`Themes/Catalogue/`) must state every value; your files may state any
subset, the rest deriving once through `GhosttyThemeMapping`.

`ThemeLibrary` reads `~/.lf/themes/` (honoring `LF_HOME`) and Ghostty's theme
folder (`XDG_CONFIG_HOME`). Names resolve yours → built-ins → Ghostty's.
`ThemeConfig` reads/writes only the top-level `theme:` line of `~/.lf/config.yaml`.
`AppearanceStore` holds Mode plus both names in memory and one transient draft of
selection and staged file writes/deletes. Apply writes the files, then the line;
Cancel discards. A one-second fingerprint poll reloads outside edits. No
UserDefaults, Rust DTO or database change; Rust ignores the `theme` key.

### Editor and import boundary

Edit exposes app surfaces/text/actions/focus, Work/file selection, status
ink/fill, terminal named and indexed colors via native ColorPicker. The filename
is the name; appearance is fixed per file. Import reads one local UTF-8 file of at
most 256 KiB in Ghostty syntax, requiring six-digit RGB background and foreground.
Named/quoted/conditional colors fail with line/key; commands, fonts, includes,
keybindings and theme references are reported and never followed.

### Palette hierarchy and readability

Ordinary app text is desaturated independently of terminal syntax. Dark component
surfaces lighten slightly rather than fall into black. Secondary/tertiary steps
remain distinct where contrast permits. Linen/Hearth/companions use neutral
heading emphasis; original Loopflow Light keeps its accepted burgundy identity.

`accentInk` owns emphasized action text and keyboard focus. Solid controls own
separate base/hover/pressed fills and accentText; actual hover state is wired.
Links/ordinary actions use text roles. Imports seed action/Working from ANSI 4,
Waiting from 3, failure from 1, success from 2 and shadow from 0; these are color
seeds, never activity evidence. Authored companions choose independent accents
across amber, rose, violet, green, teal and blue families. Unknown stays unknown.

Work selection keeps the accepted filled geometry without a new stripe. Its app
fill is calibrated independently from terminal selection: 3:1 distinction and
4.5:1 selected text. File selection keeps its neutral tint/edge. Ordinary text
requires 4.5:1 across app surfaces and actual selection composites; meaningful
focus/selection/status indicators require 3:1. Default terminal/cursor/selection
text requires 4.5:1. ANSI/dim shortfalls warn without altering source colors;
personal faint opacity and truecolor output limit those estimates.

## Concrete risk findings and alternatives

Review against [Refactoring UI](https://refactoringui.com/previews/building-your-color-palette)
and [Radix scales](https://www.radix-ui.com/colors/docs/palette-composition/understanding-the-scale)
found all 22 earlier companion palettes sharing accent with heading emphasis;
14 also shared Working ink. All had equal secondary/tertiary text; all 14 dark
companions pushed component surfaces toward black. TokyoNight body `#C0CAF5` and
Mocha `#CDD6F4` looked highlighted; Mocha's large Work selection had 12.95:1 fill
contrast. New roles/records quiet body text, separate actions/headings and retain
accessible selection. Contrast passes establish legibility, not taste.

A selection edge could permit subtler fills, but it changes Jack's accepted
geometry. Keep 3:1 filled selection. A universal blue→green replacement would
repeat the same problem; curate several hue families instead. Runtime catalogue
conversion, separate app/terminal selectors and live source-file rereads were
rejected in favor of complete theme files with one writer.

Pinned Ghostty supports config clone, compiled RGB readback and app/surface updates
without replacing PTYs. Updates return void: no atomic renderer-wide commit proof.
The baseline includes personal non-color settings; normalized colors apply last,
clearing Ghostty's independent theme selector. Existing and hidden surfaces share
updates. Reload clears key-table/sequence state and preserves adjusted font size.
Environment/resource setup now precedes ghostty_init, fixing the prior borrowed
environment-slice startup crash. Headless initialization regression passes; this
is not a crash-free sustained-use claim.

#1559 owns cell fills before glyph correction, copy receipts and finite Find cues.
Swift owns gutter/header/search/native editor chrome. Its lf7 binary lacked our
fixed-RGB getter patch; both patches now ship together in immutable
`GhosttyKit-a60e9e2-lf7-theme-readback1.xcframework.zip`. The public download matches
`19d6f6e2f93cf3d967aaaa9761c6ee1995683381ac432598d7b0496610934ce9`.
Zig, offscreen Metal, GLSL and universal build pass. No dropped validation, dual
renderer or local-only dependency pin is acceptable.

Success means a person edits/imports a favorite palette once and the entire
retained workspace follows comfortably. Failure would be unreadable role pairs,
accidental loss of your edits, or colors changing only after editor/text
recreation. Complete theme files, reversible drafts and retained-view checks
address these mechanisms; native review still determines comfort.

## Integration and deletion cut

Keep existing AppearanceStore/AppAppearance, Work selection, workspace/pane pools,
file documents and Ghostty owners. No client recreation, theme-keyed view identity,
new Work authority, status inference, installation or execution migration.

Deleted: static terminal colors, hardcoded semantic branches, runtime Ghostty
catalogue, old appearanceMode writer, duplicate overlay/readback maps, the slot
library with Reset and migrations, `appearancePreferences` UserDefaults, JSON theme
records and 15 retired catalogue entries. Nothing remains to delete.

The explicit stack-parent repair replaces refusal to change a Task parent with
CAS against the observed parent; its focused Rust regression/build/Clippy passed.

## Decisions for theme files (October 15)

Jack chose no slots, fixed 12/12 built-ins, theme files in `~/.lf/themes` and
Ghostty-syntax `theme:` in `~/.lf/config.yaml`; Mode writes a pair (System) or one
name (Light/Dark). Implementation choices are in [questions](questions.md).

- **Catalogue.** Light: Loopflow Light, Linen, Catppuccin Latte, Gruvbox Light,
  Flexoki Light, Noctis Lilac, Manila, Clay, Pewter, Frost, Vellum, Celadon. Dark:
  Loopflow Dark, Hearth, Catppuccin Mocha, Gruvbox Dark, Solarized Dark, Flexoki
  Dark, Night Owl, Onyx, Rosewood, Aubergine, Moss, Phosphor. Night Owl and
  Noctis Lilac use their MIT upstream colors. `Sources.md`/`Licenses/` updated.
- **Separate Task candidate (not here):** Jack expects user config to override
  repo config generally; today repo wins for scalars (`config.rs`).
- **Not in scope:** Liquid Glass, keyboard browsing/search and theme export.

## Remaining

Native review judges the 15 new companion palettes (generated from upstream
terminal colors with a chosen accent family each), tile actions, editor
usability and live outside-edit reload. Gate runs `uv run python scripts/test.py
--swift --loopflow` once with multiplatform/Xcode resource checks. Two inherited
WorkCacheTests expectation failures remain open gate findings. Product KRs still
require real sustained-use evidence.

Check (2026-10-15, after compress): `scripts/test_desktop.sh --filter 'ThemeTests|ThemeWorkflowTests|WorkPaletteTests|AppAppearanceTests' -Xswiftc -gnone --jobs 4` PASS (31 tests); gate and native judgment deferred.
