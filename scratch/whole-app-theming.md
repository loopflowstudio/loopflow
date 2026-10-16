# LOO-488 — Whole-app theming

Status: **accepted for implementation through demo review**, October 15, 2026.
Jack Heart approved implementation, subsequently requested the palette redesign,
and authorized launching a separate preview. Native visual judgment remains open;
no landing, installation or replacement of running clients is implied.

**Stack:** Jack's latest direction is LOO-487 / [PR #1559](https://github.com/loopflowstudio/loopflow/pull/1559), superseding direct stacking on LOO-462/#1531.
The Task records that parent; source `d6743d844fbac89693e1e25ad3e9c2a98fe9fe02`
is integrated at `ba55df3415`. Its renderer/navigation/pane contract is summarized
in [terminal polish](terminal-polish.md). Parent lands first; the separate
[Workflow-loading hold](workflow-loading.md) survives.

## Problem and accepted decisions

Jack wants a coherent workspace across navigation, headers, controls, file/diff
editors and terminal panes. Themes must preserve readable selection, focus, actions
and activity while switching retained Repo/Wave/Task contexts without losing
clients, drafts, focus ownership or layout.

October 15 decisions, in order:

- Complete themes control every LoopflowPalette role; explicit aliases are allowed.
  Ghostty RGB imports convert once into the full application contract. No hardcoded
  burgundy survives outside an authored preset.
- Independently assign any theme to Light and Dark; Mode is Light, Dark or System.
  One header row shows Mode with small label, then Light/Dark assignments. Explicit
  Light/Dark mode dims and disables the inactive assignment; System enables both.
- Keep **Loopflow Light** as the accepted original cream/burgundy design.
  Jack requested returning the old slate **Loopflow Dark**, replacing pink with a
  burgundy-related color. It keeps burgundy buttons; warm bronze now supplies
  heading/focus ink (`#CEBFA9`) and Work selection (`#AB9B86`). Exact new values
  remain review judgment.
- The quieter redesigned pair is **Linen** (warm neutral/green) and **Hearth**
  (warm charcoal). Jack accepted these names after rejecting Neo/Modern Loopflow
  branding. Linen retains its previously previewed `neo-light` internal ID so
  saved selections survive the display-name change.
- Jack then capped the library at **24 slots**, superseding 26 choices and unlimited
  appended imports. Keep the four themes above plus 20 companion palettes. Remove
  the near-duplicate Catppuccin Frappe/Macchiato catalogue entries; preserve selected
  old entries through one-time migration, not additional visible slots.
- Jack asked for a wider, more diverse companion search (draft proposal under
  review; 24 slots unchanged). Naming: keep a popular upstream theme's name where
  reuse is clearly permitted; otherwise use one consistent scheme and prefer
  simple, short names. No product/trademark names (GitHub, iTerm2, Atom, Ubuntu).
  Renamed themes take one-word material names like Linen and Hearth.
- Jack proposed **12 Light + 12 Dark** (Loopflow Light, Linen + 10; Loopflow Dark,
  Hearth + 10) so each assignment picker lists only its own appearance. This
  supersedes "either assignment accepts any palette". Draft companion set and
  bench: `scratch/theme-catalogue/index.html`. Import must match the slot's
  appearance (Jack, October 15): a dark file in a light slot previews an
  explanation pointing to the Dark picker, and Apply stays disabled.
- Jack: bias toward excluding anything not 100% clearly licensed. The
  iTerm2-Color-Schemes MIT covers the collection only; each theme keeps its
  author's license. Excluded: Modus (GPL-3.0), Monokai (no license), Man Page
  (Apple), Ubuntu (Canonical), Solarized Higher Contrast (unlicensed gist).
  Ship Night Owl and Noctis from their MIT upstream colors, not catalogue ports.
  Mark Badolato's own themes are MIT on their terminalthemes.com pages.
- Jack: Loopflow can target macOS 26 (October 15). Liquid Glass (a Solid/Glass
  material axis tinted by the theme, chrome only) is separate work, not LOO-488;
  Jack sees no reason to pursue it now, so no Task is filed.
- Jack approved the license-clean 12 Light / 12 Dark choices in
  `scratch/theme-catalogue/index.html` (October 15).
- **Edit + Reset**, no separate Duplicate. Any slot can be renamed, recolored or
  replaced by importing a theme. Reset restores its original bundled name and
  colors. All these changes participate in the chooser's existing Apply/Cancel
  transaction. Reset-to-original-slot is the reversible implementation choice
  resolving what “reset” means after an import.

The earlier owner survey, alternatives and readback evidence live at
`94d687d00:scratch/whole-app-theming.md`; the complete-palette implementation and
source audit are at `b413b95bf2:scratch/whole-app-theming.md`. Uncommitted prior
palette-review evidence was preserved in `/tmp/loo488-plan-before-neo.md`.

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

**Current preview:** `/tmp/Loopflow Palette Library Preview.app` was rebuilt
after syncing main v0.13.17 (#1559 landed; the 0.13.15 `lf` could not open the
0.13.16-migrated database), signature-verified and relaunched at Jack's request. It starts in restored Loopflow
Dark and contains the 24-slot editor. Its separate preview preferences assign Linen
to Light and Loopflow Dark to Dark. Source copies were hash-verified before signing
(`/tmp/loo488-library-package-proof.json`); process liveness was observed. No native
rendering or hands-on editor acceptance is claimed. Earlier previews remain intact.

```sh
open -n "/tmp/Loopflow Palette Library Preview.app" --args --repo /Users/jack/src/loopflow
```

## Chosen architecture and source of truth

`ThemeDefinition` contains identity/name, complete `PaletteDefinition` and a
`TerminalPalette` with all 256 indexed colors. SwiftUI/AppKit colors and Ghostty
config derive from these explicit sRGB values. `LoopflowPalette` is the projection,
not another color source. `ThemeRecord` is a complete editable/imported snapshot.

The 24 bundled JSON records define fixed slot identities and Reset baselines.
`AppearanceStore` persists one versioned `appearancePreferences` value: Mode,
Light/Dark slot IDs, and complete replacements for modified slots. There is no
appendable custom-theme catalogue. Lookup chooses one saved replacement or its
bundled baseline. Current preference decoding requires every field and rejects
unknown/duplicate slots. No per-Task/repository palette, Rust DTO or database change.

One transient draft holds assignments and all replacement records. Edit and import
replace the selected slot without changing its ID, so both assignments continue
following that slot. Reset removes its replacement. Apply validates the complete
draft library and writes it once; Cancel/close/Escape discard the whole draft.
Quick Mode changes discard preview and retain applied slot contents. New windows
share preview; relaunch reads only applied preferences.

Version 5 migrates old preferences once. Versions 1/2 derive missing app palettes;
version 3 preserves explicit colors while splitting accentInk/pressed roles;
version 4 preserves complete imported snapshots. Old imports occupy available
slots while preserving assigned built-ins and remapping both assignments. Selected
retired Catppuccin records become replacements from `Themes/Migrations/`; current
preferences never read those files. If old records cannot fit without loss, keep
original bytes and show recovery instead of silently discarding themes. Missing,
corrupt or unsupported saved data likewise remains available until a valid Apply.

### Editor and import boundary

Edit exposes name, native light/dark appearance, app surfaces/text/actions/focus,
Work/file selection, status ink/fill, terminal named colors and indexed colors.
Native ColorPicker owns color input; displayed RGB values are selectable text.
Alpha is supported only for the existing file-selection paint. Tone stroke/text
remain explicit ink aliases. Reset restores the entire original record, including
name. Invalid names or unreadable edited colors disable Apply; the last valid
appearance remains visible while the draft explains the failure.

Import replaces the selected slot, never adds one. One local UTF-8 file, at most
256 KiB. A complete Loopflow JSON record must decode every palette/terminal field;
its incoming ID cannot replace the destination slot identity. Ghostty input accepts
blank lines/full-line comments and `key=value`, requiring six-digit RGB background
and foreground. Optional cursor/selection colors and decimal palette indices
0–255 retain their supplied values. Repeated keys use the final value with notices.
Missing values derive from the polarity's Loopflow preset and foreground/background.
Named/quoted/conditional/cell-relative colors fail with line/key. Commands, fonts,
includes, keybindings and theme references are reported and never followed.
Read/parse failures preserve the draft and applied library. Saved files are never
reread. Export, online discovery/sync and a freeform shader editor remain excluded.

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
rejected in favor of complete saved records. The fixed library supersedes
Duplicate plus an unlimited custom section, without a second preference writer.

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
accidental loss while overwriting slots, or colors changing only after editor/text
recreation. Complete records, reversible drafts, Reset and retained-view checks
address these mechanisms; native review still determines comfort.

## Integration and deletion cut

Keep existing AppearanceStore/AppAppearance, Work selection, workspace/pane pools,
file documents and Ghostty owners. No client recreation, theme-keyed view identity,
new Work authority, status inference, installation or execution migration.

Completed removal: static terminal colors; hardcoded semantic light/dark branches;
unused burgundy helpers/StatusColors; runtime Ghostty catalogue; old appearanceMode
writer; truecolor default zsh prompt; diff text-only recolor guard; duplicate
Ghostty overlay/readback maps. Native text/caret/selection/link and unchanged diffs
consume the palette without losing selection. Custom prompts stay owned by shells.

Current deletion: `terminalHover/Selection/Failure` palette fields and JSON values,
GhosttyCommandBlockStyle/Swift cell-fill assertions, old copy overlay, append-only
import/pending-import arrays, and the obsolete chooser-only single-theme path.
Preserve renderer, clipboard, retained Work, name/assignment migration and import
error tests. Frappe/Macchiato records are bounded migration inputs only.

The explicit stack-parent repair replaces refusal to change a Task parent with
CAS against the observed parent; Task/PR/checkout/history survive, cycles and stale
writes remain rejected. Its focused Rust regression/build/Clippy passed.

## Next: approved catalogue and 12 Light / 12 Dark pickers

Jack approved this on October 15 during demo review. Build it in this Task.

**Redesign (October 15).** Jack didn't love Edit and Import as separate buttons.
He chose **no slots**, and **Ghostty-syntax text config** in the existing
`~/.lf/config.yaml`. Catalogue and license decisions stand.

- **Built-ins are fixed.** The 24 catalogue themes (12 Light, 12 Dark) are
  read-only and always available by name.
- **Your themes are files.** Ghostty-format theme files in `~/.lf/themes/`
  (writable), plus read-only `~/.config/ghostty/themes/`. Ghostty RGB keys give
  the terminal; app roles derive via `GhosttyThemeMapping` unless the file sets
  Loopflow app-role keys. A file named like a built-in overrides it, as Ghostty's
  user directory does.
- **Picker.** Two pickers, Light and Dark, each listing built-ins then Yours. The
  selected tile carries its own actions: **Customize** (a built-in writes your
  override copy; one of yours opens the editor in place), and **Delete** for yours
  ("Revert to built-in" when it overrides one, which replaces Reset). **Import
  file…** sits at the end of Yours and accepts only the picker's appearance.
- **Config.** `theme: "light:Linen,dark:Hearth"` (Ghostty value syntax) in
  `~/.lf/config.yaml`. Names resolve to your files first, then built-ins, then
  Ghostty's themes folder. The repo `.lf/config.yaml` never sets the theme; a
  repo can't recolor a multi-repo window. Absent means Loopflow Light/Dark.
- **Picker doesn't fight config.** The file is the only store. Preview is in
  memory; Apply rewrites only the `theme:` line and leaves the rest of the file
  intact; Cancel writes nothing. The app watches the file and applies outside
  edits live. An unknown name keeps the last good theme and shows the name and
  the fix. UserDefaults `appearancePreferences` is deleted, with no migration
  (solo dev, no compat).
- **Ghostty's config.** It still loads for non-color settings. Its `theme =` is
  ignored; Loopflow's config owns colors. Ghostty's file is never written.
- **Deleted:** slots, the 24-slot editable library, slot Reset, slot migration,
  and the UserDefaults appearance library.
- **Mode writes the format** (Jack, October 15). System writes the pair; Light
  or Dark writes only that side's name, dropping the other side from the file.
  The other picker is dimmed in Light or Dark mode. Returning to System restores
  the other side from this session when known, otherwise Loopflow Light/Dark.
- **Separate Task candidate (not here):** Jack expects user config to override
  repo config generally; today repo wins for scalars (`config.rs`), and some keys
  are repo-only by design.

- **Catalogue.** Light: Loopflow Light, Linen, Catppuccin Latte, Gruvbox Light,
  Flexoki Light, Noctis Lilac, Manila, Clay, Pewter, Frost, Vellum, Celadon. Dark:
  Loopflow Dark, Hearth, Catppuccin Mocha, Gruvbox Dark, Solarized Dark, Flexoki
  Dark, Night Owl, Onyx, Rosewood, Aubergine, Moss, Phosphor. Upstream sources and
  reasons are in `scratch/theme-catalogue/index.html` (`build_page.py` table).
  Each new theme gets a complete hand-curated app palette that passes WorkPaletteTests.
- **Sources.** Take Night Owl and Noctis Lilac terminal colors from their MIT
  upstreams (sdras/night-owl-vscode-theme, liviuschera/noctis), not catalogue
  ports. Solarized Dark uses the original colors without the iTerm2 name. Update
  `Sources.md`/`Licenses/`: author license per theme, Badolato themes cite their
  terminalthemes.com MIT page, Neovim is Apache-2.0; delete notices for removed themes.
- **Appearance split.** Every theme is Light or Dark. Each picker shows only its
  own; Import of the wrong appearance previews an explanation naming the other
  picker and leaves Apply disabled. Delete "either assignment accepts any
  palette" from code and README.
- **Not in scope:** Liquid Glass, keyboard browsing/search and theme export
  (Ghostty comparison ideas, undecided).

## This slice and Done when

Deliver the approved catalogue, appearance-split pickers, theme files and
`theme:` config together, deleting slots, Reset, slot migration and the
UserDefaults library in the same diff. Update sources/license notices and
Desktop usage beside code.

Headless focused acceptance:
`scripts/test_desktop.sh --filter 'ThemeTests|ThemeWorkflowTests|WorkPaletteTests' -Xswiftc -gnone --jobs 4`.
Expect 12 built-ins per picker; your files override and revert by name;
appearance-mismatched import is refused with its explanation; Apply writes
only the `theme:` line in the expected format for each Mode; Cancel writes
nothing; outside edits reload; unknown names keep the last good theme; repo
config never sets the theme; unchanged native text and retained
Work/layout/drafts survive color changes.

Gate runs `uv run python scripts/test.py --swift --loopflow` once, with parent
feedback/pane/navigation suites and multiplatform boundary checks. Two inherited
WorkCacheTests expectation failures remain open gate findings. Unavailable native
checks belong to capable demo/CI, never an earlier-step permission dialog.
Product KRs still require real sustained-use evidence; no timing target or external
project progress is inferred from these checks.

Review repairs: capture the slot ID before mutating its draft to avoid Swift
exclusivity failure during import/reset; reject malformed JSON terminal lengths
before preview indexing; validate actual file-selection composites; clamp native
picker colors to sRGB. The editor uses platform color entry instead of maintaining
a second partially valid hex draft. A headless view test exercises rename and Reset;
full-library tests cover repeated replacement, both assignments, Cancel, relaunch,
retired presets and over-capacity legacy data without silent deletion.

Check (2026-10-15): `scripts/test_desktop.sh --filter 'ThemeTests|ThemeWorkflowTests|WorkPaletteTests' -Xswiftc -gnone --jobs 4` PASS (30 tests, `/tmp/loo488-library-tests.log`); combined Ghostty/public checksum, 9 Python artifact/dev tests, platform boundaries, 24-slot/source-color audit and preview signature/launch PASS. Broader gate and native editor judgment remain open.
