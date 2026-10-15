# LOO-488 — Whole-app theming

Status: **accepted for implementation through demo review**, 2026-10-15.
Jack Heart approved the design with “looks good to me” after confirming the
LOO-462 stack. The core path and complete Ghostty readback are implemented; native visual
judgment remains at demo review. Approval does not authorize landing, release, installation
or restarting running apps.

**Accepted stack, 2026-10-15:** Jack Heart directed LOO-488 to stack on LOO-462.
The Task already records LOO-462's [PR #1531](https://github.com/loopflowstudio/loopflow/pull/1531)
as its parent. The checkout base is now `7d10108eaf928ee9db3a84467d23ce4b929fa0a8`,
integrated at `ffdcf68935d85c14484f51825f1d31dc67616c56`. The subsequent design
approval authorizes implementation on this stack; the parent still lands first.

## Problem and accepted direction

Jack Heart wants one coherent Loopflow workspace: sidebar, headers, controls,
pane surfaces and terminal content. Previously fixed cream/slate chrome surrounded
an independently configured dark terminal. The intended experience is sustained
coding and switching retained Repo/Wave/Task contexts with familiar colors,
readable state and reversible changes that preserve work.
[Task](https://linear.app/loopflow/issue/LOO-488/make-loopflow-coherent-in-a-chosen-whole-app-theme).

Binding requirements from Jack's October 15 direction:

- Choose, preview, apply and restore appearance through the existing appearance
  entry point. Preserve recognizable Light/Dark options and a clear way back.
- One mapping covers app, terminal and pane chrome. Selection, keyboard focus,
  links, actions and activity retain separate meanings. Unknown remains unknown;
  terminal colors never assign Task or Session state.
- Applying or reverting preserves native clients, drafts, pane layout, focus
  ownership and Work identity across navigation and splits.
- Include 24 embedded themes, independent Light/Dark assignments and Light/Dark/System
  mode. Require complete app palettes and convert Ghostty RGB imports into that
  contract. Automated checks establish contrast/behavior; native review judges coherence.

[Reference section 2](rex-polish-reference.html#themes) proposes a themed workspace
and Ghostty import. Its Tokyo Night mock is illustrative, not an approved palette
or a requirement to reproduce Rex's catalogue/materials.

LOO-462 owns the concurrent pane islands, program headers, collapsible navigation,
attention queue, copy feedback and unobstructed search. Consume its semantic roles
and retained workspace owners; do not recreate those features. The separate
animation/shader/color work owns automatic terminal cell contrast correction.

Product alignment: preserve ongoing work, October 14's semantic distinctions and
hidden PTYs. Palette tests cannot prove sustained-use KRs. October 7's retirement
of numeric/soak acceptance remains binding.

## Demo

Choose **Appearance → Choose Theme…** with a conversation, shell and edited file
open. Preview both assignments, switch Wave/Task, expand attention, split/focus
panes and inspect selected/error/focused states. Cancel restores the applied
appearance; Apply persists assignments and Mode. Work, drafts and clients remain.
Judge real provider output and shell colors together; emitted truecolor may keep
its own appearance. Light is accepted; Dark and other palettes need native review.

## Architecture evidence and limits

The pre-implementation owner survey and upstream probes are preserved at
`74ee56847ae39997fefd82e28285cca9afa876d9:scratch/whole-app-theming.md`,
**Current architecture and risk findings**. The Appearance menu, environment
palette, retained workspace pools and editor documents remain the existing owners.
No new Work, Session or terminal-lifetime authority is needed.

Pinned GhosttyKit supports config clone, diagnostics/readback and
app/surface updates without replacing the PTY. App updates cover existing and
future surfaces, but return `void`: validation cannot prove an atomic renderer
commit. Reload clears key-sequence/key-table state and preserves manually adjusted
font size. Live rendering, unsent provider input and focus require native demo
judgment; headless view/configuration tests do not establish them.

## Accepted approach

### One local appearance owner and one resolved theme

Extend the existing Swift design types with immutable `ThemeDefinition`: identity,
name, native light/dark scheme, semantic `LoopflowPalette` and terminal colors.
Color storage uses explicit sRGB values; SwiftUI `Color`, AppKit `NSColor` and
Ghostty config text derive from them. `TerminalPalette` becomes value data within
the definition, not a static global. No Rust/CLI DTO, database, Work or Session
model changes are required.

An app-owned `AppearanceStore` holds applied selection and transient preview.
Persist one versioned `appearancePreferences` UserDefaults value containing
Mode, Light/Dark theme assignments and complete imported records. Bundled JSON
records and imported snapshots each contain a required `PaletteDefinition` plus
terminal colors. Conversion happens once on import; no runtime derivation or
source-file reread competes with the saved record. Preference scope is
local to Desktop and shared by all windows, not Task/repository-specific.

Replace Desktop's single theme selection with Mode plus Light/Dark theme IDs.
Migrate `appearanceMode` once to System / Light / Dark, writing the new value
before deleting the old key; thereafter write only the new value. Missing means
System. Preserve invalid saved bytes for diagnosis, show a recoverable appearance
notice and use System for this launch until a valid selection is applied. A missing
imported ID likewise presents a usable built-in choice without silently erasing
the unavailable selection.

`AppAppearance` remains the window integration point. All windows consume the same
effective theme and native scheme; System follows OS appearance through one
app-owned observation. Appearance changes never refresh Work, navigate, acquire
clients or change visibility. Test/capture overrides pin the whole theme rather
than only SwiftUI's color scheme.

### Picker, preview and defaults

Keep quick System, Light and Dark choices and add **Choose Theme…**. The chooser
shows theme cards, a small workspace/terminal sample, **Import Ghostty Theme…**,
**Apply** and **Cancel**. Card selection previews across the app; Apply persists;
Cancel, Escape or closing restores the applied selection. One app-wide chooser
owns preview so windows cannot compete. A quick menu choice applies directly and
ends preview. New windows use the preview; relaunch uses only the applied choice.

Use **24 bundled themes plus imports**, with independently assigned Light and Dark themes. System selects between those assignments. Keep
recognizable cream Light and slate Dark app palettes and typography. Extend each
into coherent terminal/pane colors: cream terminal for Light and slate terminal
for Dark. The accepted design changes the stock charcoal terminal; exact colors
remain subject to native demo review. A Classic preset preserving the old pairing
was considered but is outside scope. Jack subsequently requested an embedded catalogue;
no online browser, sync or material effects.

### Semantic mapping and readability

`LoopflowPalette` owns structure, text, selection, focus, links, actions and complete
tone styles (ink/fill/stroke/text), including terminal block/search chrome.
`WorkTone` continues mapping existing state to those roles, without theme hex
literals. Terminal ANSI indices remain colors, never activity enums.

For imports, preserve terminal background/foreground and supplied indexed colors.
Derive app surfaces from the background hue with a restrained lightness ladder;
choose light/dark by its contrast polarity. Derive secondary text from foreground,
neutral file selection from the surface/text ramp. `GhosttyThemeMapping` uses
ANSI 4 for accent/focus/Working, 3 for Waiting/Blocked, 1 for failure, 2 for success
and 0 for shadow. Terminal selection supplies Work selection, with independently
readable primary/secondary text. These are import color seeds, never activity
authority. Labels and existing shapes preserve meaning when hues are similar.
The parent’s Ink on Paper selection geometry remains; burgundy remains part
of Loopflow’s own presets, never imposed on other themes.

Every non-alias role is required by Codable, including status ink/fill and fill
opacities; missing roles fail decoding. Explicit aliases are focus→emphasis,
selection edge/links/actions→text roles, action hover→surface element and
status stroke/text→status ink. No alias introduces a fixed color. Bundled records
are authored complete JSON; `.ghostty` files are import inputs only.

Resolve deterministically in sRGB. Bound surface steps so app text can share one
contrast polarity; choose the nearest lighter/darker role value that passes all
composed surfaces. Reduce optional fill opacity when it breaks contrast. An
impossible mapping is a named import error, not a quiet substitution of an
unrelated stock palette. Built-ins use explicit reviewed values through the same
validation. Exact ladder spacing/lightness adjustments remain visual review work;
the acceptance thresholds are fixed below.

Ordinary app text, terminal default text and selection text require 4.5:1;
meaningful selection/focus/status indicators require 3:1. Include actual hover,
selected, chip, failed-shell-block and search compositions. Terminal dim/ANSI
shortfalls appear in the preview with original colors retained. Do not reject an
otherwise usable import solely because ANSI 0/8 or application truecolor output
fails. Low-contrast default or selection text prevents Apply with the pair and
ratio named. This Task supplies a complete terminal palette to later renderer
correction; it does not implement per-cell correction.

### Exact import boundary

One local UTF-8 file, maximum 256 KiB. Parse blank lines, full-line comments and
`key = value`. Require six-digit RGB background/foreground; optionally accept
cursor-color, cursor-text, selection-background/foreground and decimal indexed
`palette = N=RGB` (0–255). Repeated keys use the final value with diagnostics.
Missing ANSI uses the polarity’s Loopflow defaults; cursor/selection derive from
foreground/background. Preview reports supplied, derived and omitted settings.

Invalid recognized colors name line/key. Named X11, quoted/multiline, conditional
and cell-relative values are unsupported. Non-color settings, includes and theme
references are reported and never followed/forwarded. Cancel/read/parse failure
preserves applied preferences. Basename supplies display name; independent IDs
prevent collisions. Saved snapshots never reread their original files.

### Ghostty integration and retained views

Keep personal non-color Ghostty settings and existing embedding policies.
Initialize a non-appearance base config through the existing loader; preview
clones that baseline rather than rereading external files or changing fonts and
keybindings. Apply the normalized overlay last, clear Ghostty's independent
`theme` selector, and explicitly set all managed foreground/background, cursor,
selection, search and indexed colors. Use opaque sRGB terminal surfaces.
Diagnostics/readback must prove personal color settings and conditional themes
cannot override the Loopflow selection. Preserve TERM, shell integration, padding
and embedding keybindings. The baseline is runtime configuration, not a second
persisted theme record.

Build/validate the candidate before changing effective appearance. Update the
Ghostty app config and light/dark scheme on the main actor once per distinct
candidate, covering hidden surfaces and future creation. SwiftUI and native
styles consume the same value. Retained views observe changes for backing layers,
blocks and search even while their SwiftUI mounts are hidden. Reuse existing pools
and view lifetimes; appearance owns no second terminal registry.

Change only Loopflow's default zsh prompt to indexed color (slot 5).
New shells then follow palette changes without injected input. Pre-upgrade shells
already holding truecolor prompts, custom prompts and emitted truecolor output
remain application-owned text; do not restart shells or rewrite scrollback.
Config allocation/validation failures leave the applied theme intact and show an
appearance error. Native update errors retain clients and existing diagnostics;
the API supplies no renderer-wide success acknowledgement.

## Alternatives and palette review — October 15

Keep one complete theme record, bounded snapshot imports and retained terminal
updates. Reject separate app/terminal selectors, source files as live authorities,
lookup-time derivation and surface recreation. Full Ghostty configuration import
remains outside scope. Earlier alternatives: `94d687d00:scratch/whole-app-theming.md`.

Jack Heart accepted Light, rejected Dark, and described other themes as too blue
and highlighted; green was tentative, not a blanket replacement request. The
local warm-charcoal Dark candidate passes contrast but is not packaged or visually
accepted. Light remains unchanged. No neutralization of the other 22 has landed.

Review against [Refactoring UI](https://refactoringui.com/previews/building-your-color-palette)
and [Radix scales](https://www.radix-ui.com/colors/docs/palette-composition/understanding-the-scale):

- Complete explicit records pass; hand-tuned role hierarchy remains missing.
- All 22 converted palettes alias emphasis to accent; 14 also match Working ink.
  Wave headings, focus and global tint spread the same highlight. Separate these
  responsibilities; ordinary headings should use quiet theme text.
- Terminal foreground also tints body text: TokyoNight `#C0CAF5`, Mocha `#CDD6F4`.
  Reduce app-text chroma independently; preserve terminal originals.
- All 22 have identical secondary/tertiary text. Restore visible hierarchy without
  sacrificing legibility; do not blindly darken text on dark surfaces.
- All 14 converted dark palettes darken component surfaces toward black. Fixed
  RGB percentages are not a reviewed surface/state scale. Tune component states.
- Mocha Work selection is `#F5E0DC` on `#1E1E2E` (12.95:1); TokyoNight 3.03:1.
  Terminal selection is an unreliable large-row fill. Quieter dark/imported
  selection needs a deliberate distinguishing cue, not lower accessibility.
- `accent` is adjusted for text contrast then reused as a button fill; separate
  strong fill from readable accent ink. Focus→emphasis and status stroke→text
  aliases are too broad for independent tuning.
- Outline/solid buttons use hover-named roles on press, without hover handling;
  normal/hover/pressed states need coherent shade choices.
- Contrast passes prove readability only. Review real sidebar, selected/error/focus
  combinations and controls together; preserve Light’s accepted composition.

Proposed order: quiet ordinary text/headings; separate focus/action/status roles;
retune selection and surface states; then calibrate each bundled palette.
These findings are recommendations, not approval of exact colors or weaker checks.

## Scope and deletion cut

Affected consumers: all Desktop roots/sheets; navigation/attention; Task/Workflow
headers/graph; pane controls/placeholders; file/diff editors; Ghostty config,
native background/search/block/header decoration and shell bootstrap. Update
`swift/README.md` and Desktop semantics in `VISUAL_DESIGN.md`. Shared color types
remain iOS-buildable; an iOS picker, website themes, Rust preferences and wire
changes are excluded.

**Completed deletion cut:**

- Static `TerminalPalette` colors and `GhosttyManager.loopflowConfig` color
  literals removed; non-color defaults retain their existing owner.
- Hardcoded semantic light/dark branches in `LoopflowPalette`/`WorkTone`, graph/diff
  literals, fixed native decoration colors and forced `.dark` pane environments.
  Consumers now use resolved roles.
- Unused `LoopflowPalette.deepWine`, static burgundy/cream helpers and
  `Models/StatusColors.swift` removed; Light/Dark are ordinary bundled records.
- Runtime `.ghostty` catalogue and hardcoded `TerminalPalette.standard` values
  replaced by complete JSON. Standard import defaults read the bundled records.
- Diff text-only update guard replaced: a palette change recolors unchanged text
  without replacing its selection. Native editor caret/selection/link styling,
  history controls and Find controls consume the palette. Fixed shadow/shimmer
  colors and secondary system-label overrides removed.
- Obsolete TaskFiles test for the removed `work-toolbar`/checkout chip deleted;
  its traversal crashed on a missing workspace environment before theme tests.
  File routing/draft/read/save tests remain.
- Desktop's old `appearanceMode` writer, obsolete `AppearanceMode.resolvedTheme`
  path and tests exclusive to that representation after preference migration.
- `LoopflowZshBootstrap.accent` truecolor substitution removed; custom-prompt
  detection, header markers, shell integration and TERM behavior remain.
- The lf3 unsupported-readback assertion replaced by compiled-color checks
  and rejection coverage for every managed color key.
- Stored `AppearanceStore.chooserOpen`; preview selection already owns the
  transaction. Repeated window-root fixtures removed; shared-modifier assertions remain.
- Duplicate Ghostty color mappings in overlay generation and readback validation;
  both now consume `TerminalPalette.ghosttyColors`.

Retain contrast, state-wording, shell-block and workspace/input tests, extending
them to themes. Forbidden: static/dynamic palette split, hidden terminal-only
color override, mirrored preference writes, theme-keyed `.id` on Work or terminals,
new Work authority, client restart or renderer correction implementation.

No workspace layout redesign, downloader, shader/material system, color editor, font
preferences, execution migration, installation or release. Keep the inherited
`scratch/workflow-loading.md` parent hold intact; this Task does not supersede it.
LOO-462's `7d10108eaf` integration adds Jack Heart's accepted Ink on Paper
selection, drill-in navigation and outer Work layouts. Those owners remain intact.
WorkStyle's fixed selected-color extension is removed; selected fill and text
resolve through LoopflowPalette. ThemeWorkflowTests now uses the sidebar's
production collapse/restore controls instead of the removed WorkBreadcrumbBar.

## Current implementation and remaining review

The earlier implementation receipts, readback extension, parent integration and
review fixes are retained at `94d687d00:scratch/whole-app-theming.md`. They establish
headless configuration, palette and retained-owner behavior, not live input or
mounted SwiftUI propagation. Native visual/focus review and the two inherited
WorkCacheTests gate failures remain open.

**Jack Heart, October 15 — accepted refinement:** include roughly 12–48 embedded
themes; any bundled or imported theme may be assigned independently as the Light
and Dark theme. The third setting is Mode: Light, Dark or System. System selects
the corresponding assignment from macOS appearance. Assignment is unrestricted by
a palette's brightness. This supersedes two-built-ins-only and a single selected
theme; it does not approve landing or claim demo acceptance.

Implementation choice: ship 24 offline themes: Loopflow Light/Dark and 22 pinned
palettes converted from iTerm2-Color-Schemes. Source links and license notices
travel with the bundled resources. Background/foreground/ANSI are unchanged;
marked cursor/selection text adaptations meet 4.5:1. Imports remain exact snapshots
with the existing diagnostics. Both assignments and Mode persist together in the
existing appearance preference record, version 3; versions 1/2 are migrated once,
retaining import IDs, both assignments and Mode while completing old palettes. No parallel preference
writer or download service.

Jack Heart’s October 15 chooser refinement puts Mode, Light theme and Dark theme
in one header row with small labels above. Light/Dark mode dims and disables the
unused assignment; System enables both. Changing Mode selects its relevant slot.
Choosing an enabled target previews it without changing Mode. Apply stores both assignments
and returns to the mode-resolved appearance; Cancel/close restores all applied
settings. Quick mode choices retain assignments. All themes can occupy either
slot, including the same theme in both. System changes choose slots from OS
appearance, not a selected palette's brightness. Missing/corrupt records retain
bytes and show recovery; invalid colors cannot apply either assignment.

**Startup repair:** Ghostty borrowed an environment slice invalidated by later GUI
setup. Environment/resource configuration now precedes `ghostty_init`; its headless
regression passes. Crash evidence and unwinding remain in
`/tmp/loo488-plan-before-palette-review.md`; no native crash-free claim follows.

**Jack Heart, October 15 — accepted correction:** every view uses LoopflowPalette;
every theme must supply its complete palette, with explicit aliases permitted.
The unchanged burgundy was a hardcoded mapper seed, not merely a missed view.
Full records now own accent, focus, selection, status, surfaces and shadow.
Jack Heart corrected the default conversion: Loopflow Light must retain the regular
design. Light preserves its original design; Dark’s subsequent rejection and candidate
are recorded above. Only imports use the mapper. Native diff updates had also skipped unchanged text;
color-only updates now preserve content and selection. No Flow has advanced.

Both assignments are validated even when inactive. Native 8-bit compositing
requires a 4.55:1 mapping margin to retain 4.5:1; all 24 palettes pass.

## Acceptance and remaining gate

The complete scenario and edge-case matrix are preserved at
`94d687d00cbd4570fc2f433ff94ac7d57d6e9fec:scratch/whole-app-theming.md`,
**Done when and unattended gate**. The implementation evidence above covers the
headless owners/views: migration, four appearances, preview/Cancel/Apply,
restoration, navigation, retained drafts/layouts/views, import errors and compiled
Ghostty colors. It does not establish mounted propagation or live terminal input.

Gate remains `uv run python scripts/test.py --swift --loopflow`: full headless
Swift suites, multiplatform boundaries and Xcode fallback build. The two inherited
WorkCacheTests failures remain unresolved gate findings. Unavailable host checks
belong to capable CI; display suites are explicitly skipped, never counted as
passes. Publication review found no further publication blocker; it did not prove
native acceptance.

Check (2026-10-15): `scripts/test_desktop.sh --filter 'ThemeTests|WorkPaletteTests|commandBlockFillContrast' -Xswiftc -gnone` — 21 tests PASS for charcoal Dark; palette inventory confirms review counts above. Earlier chooser 3 tests PASS. Native review/broader gate remain outstanding.

## Demo handoff — 2026-10-15

The signed preview below contains the restored presets and integrated chooser,
not the newer charcoal Dark candidate. It retains the preview identity/preferences
and bundled CLI. No installation or live-client restart occurred.

```sh
open -n "/tmp/Loopflow Restored Theme Preview.app" --args --repo /Users/jack/src/loopflow
```

Appearance → Choose Theme… exposes Light theme, Dark theme and Mode. Compare
Alabaster/Dracula with a file and diff open: selection, focus, controls and
unchanged diff content must all follow the theme. Apply or Cancel preserves
work. Import samples remain in `/tmp/Loopflow Theme Samples/`. Headless checks
prove configuration and retained owners, not live input or visual coherence.
