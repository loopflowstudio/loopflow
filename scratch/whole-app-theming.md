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

Open Appearance → Choose Theme. Compare Loopflow Light, the restored Dark, Linen
and Hearth with the same conversation, shell and unsaved file open. Edit a name
and colors, import over that slot, then Reset: original name and colors return.
Cancel restores the previously applied library and both assignments. Repeat and
Apply; relaunch restores all changes in exactly 24 slots.

Switch Work, open temporary/pinned navigation, inspect Needs you, split/focus panes
and revisit unchanged diff text throughout. Clients, drafts, selections and layouts
remain. System changes choose Light/Dark assignments, not a palette's brightness.
Live provider output and visual comfort belong to review; truecolor output can
retain its own colors. No test substitutes for that judgment.

**Current preview:** `/tmp/Loopflow Palette Library Preview.app` was built,
signature-verified and launched at Jack's request. It starts in restored Loopflow
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

## This slice and Done when

Deliver the combined parent integration, 24-slot library, palette redesign and
Edit/Reset/import cutover together. No partial infinite-import or uneditable custom
store remains. Update sources/license notices and Desktop usage beside code.

Headless focused acceptance:
`scripts/test_desktop.sh --filter 'ThemeTests|ThemeWorkflowTests|WorkPaletteTests' -Xswiftc -gnone --jobs 4`.
Expect exact 24 slots after repeated edit/rename/import/reset; both assignments
follow stable slot IDs; preview/cancel/apply/relaunch preserve full records;
invalid input preserves saved bytes; removed assigned presets migrate without
extra slots; original Light and restored Dark remain available; unchanged native
text and retained Work/layout/drafts survive color changes.

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
