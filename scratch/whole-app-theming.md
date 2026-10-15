# LOO-488 — Whole-app theming

Status: **accepted for implementation through demo review**, 2026-10-15.
Jack Heart approved the design with “looks good to me” after confirming the
LOO-462 stack. Implement the selected approach below; native visual judgment
remains at demo review. Approval does not authorize landing, release, installation
or restarting running apps.

**Accepted stack, 2026-10-15:** Jack Heart directed LOO-488 to stack on LOO-462.
The Task already records LOO-462's [PR #1531](https://github.com/loopflowstudio/loopflow/pull/1531)
as its parent, with base `c9bc78904b52fdceff46959ce011a394a28fb706`.
Keep that dependency as LOO-462 advances; land the parent first. The subsequent design approval authorizes implementation on this stack; the
parent still lands first.

## Problem and accepted direction

Jack Heart wants one coherent Loopflow workspace: sidebar, headers, controls,
pane surfaces and terminal content. Fixed cream/slate chrome currently surrounds
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
- Evaluate Ghostty import; format, catalogue size and exact presets remain design
  choices. Automated checks establish contrast and behavior; native review judges
  coherence.

[Reference section 2](rex-polish-reference.html#themes) proposes a themed workspace
and Ghostty import. Its Tokyo Night mock is illustrative, not an approved palette
or a requirement to reproduce Rex's catalogue/materials.

LOO-462 owns the concurrent pane islands, program headers, collapsible navigation,
attention queue, copy feedback and unobstructed search. Consume its semantic roles
and retained workspace owners; do not recreate those features. The separate
animation/shader/color work owns automatic terminal cell contrast correction.

Product alignment: this supports ongoing Desktop work without adding Work
authority. The current Project names sustained Desktop use and access without
crashes as KRs, with no numeric metric targets. The October 7 decision in Wave
memory retired numeric/soak acceptance; this feature neither reinstates it nor
claims those KRs from palette tests. Preserve October 14's semantic distinctions
and the memory's warning that freeing a hidden surface ends its PTY.

## Demo

In a workspace with a Task conversation, a shell and an edited file, choose
**Appearance → Choose Theme…**, import a dark Ghostty color theme, then preview it.
The outline, Task/Workflow header, controls, pane headers, shell background and
terminal palette change together. Switch to another Wave and back, expand the
attention queue, change pane focus and split the shell. Working, Waiting/Blocked
and Error remain legible beside a selected row and focus outline.

Cancel restores the applied theme without losing text or moving panes. Apply keeps
the choice across the next ordinary launch. Light, Dark and System remain available
in the same menu. Repeat with a light import. Native review inspects a real
provider's output as well as the shell; applications painting explicit truecolor
pixels may retain their own colors.

## Current architecture and risk findings

Baseline inspected: `c9bc78904b52fdceff46959ce011a394a28fb706`.

| Owner today | Finding and implication |
|---|---|
| `LoopflowApp.swift`, `AppearanceMode.swift`, `AppBootstrap.swift` | `@AppStorage("appearanceMode")` selects System/Light/Dark. `AppAppearance` supplies palette and native scheme to workspace, Portfolio, Open Work and Telemetry windows. There is an Appearance menu, not an existing settings pane. Extend that entry point. |
| `Loopflow/Design/BrandColors.swift` | `LoopflowPalette` is already the SwiftUI environment contract, but semantic colors branch on `isDark`. `deepWine` has no consumers. Extend the existing palette; do not introduce a competing app color API. |
| `WorkStyle.swift`, `FlowGraphView.swift`, `WaveLensView.swift` | Tone fill/stroke/text and some graph colors contain literal light/dark pairs. Changing only the root palette leaves them behind. |
| `TerminalPalette.swift`, `GhosttyManager.loopflowConfig` | Fixed terminal constants and 16 ANSI colors live separately. Initialization loads them, then personal Ghostty configuration, then embedding rules. Personal colors can override the terminal without updating SwiftUI or its backing layer. |
| `SessionsView.swift`, `GhosttyTerminalView.swift` | Pane chrome forces `.dark` in places. Backing layers, search highlight, command blocks/separators and directory headers have independent fixed colors. Each retained view owns its surface; each window owns its pool. |
| `LoopflowZshBootstrap` | The default prompt embeds truecolor rose. Config reload cannot recolor those bytes. Use an indexed color in the Loopflow-owned prompt; preserve custom prompts and shell hooks. |
| `TaskFilesView.swift` | Editor foreground/background already receive the environment palette; diff fills use system red/green. Recolor the retained editor without replacing its document, selection or Undo. |
| `AppAppearanceTests`, `WorkPaletteTests` | Contrast and ViewInspector tests work headlessly. The existing native window-appearance test requires a display and is skipped by the headless runner; it cannot be the sole acceptance test. |

### Live update is supported; it is not an atomic rendering transaction

SwiftPM pins `GhosttyKit-a60e9e2-lf3`. At its upstream source revision
`a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0`,
[`ghostty.h`](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/include/ghostty.h)
exports config clone/get/diagnostics and app/surface config updates.
[`App.updateConfig`](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/App.zig#L177)
iterates existing surfaces; embedded app handling copies the config for future
surfaces. [`Surface.updateConfig`](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/Surface.zig#L1776)
updates renderer/terminal configuration without replacing the PTY. Use this API,
not terminal destruction, shell commands or OSC injection.

The C update returns `void`; internal failures log rather than acknowledge an
atomic all-surface commit. Validate candidates before publication and retain the
last applied candidate. Reload clears Ghostty key-sequence/key-table state and
preserves manually adjusted font size. Live rendering, unsent provider input and
focus still require native demo evidence. No runtime renderer proof was performed
in this design edge.

### Import is feasible without adopting an entire Ghostty configuration

Themes use configuration syntax and may contain non-color settings. Adopting the
whole language would expand appearance into launch/keybinding policy. Choose a
color-only import with explicit omitted-setting diagnostics.
[Ghostty configuration reference](https://ghostty.org/docs/config/reference#theme).

Published [TokyoNight](https://raw.githubusercontent.com/mbadolato/iTerm2-Color-Schemes/master/ghostty/TokyoNight)
and [Gruvbox Light](https://raw.githubusercontent.com/mbadolato/iTerm2-Color-Schemes/master/ghostty/Gruvbox%20Light)
files each contain 16 indexed colors plus background, foreground, cursor and
selection RGB values, all within the proposed subset. October 15's source-only
probe measured default-text contrast of **10.59:1 / 10.22:1** and selection text of
**5.64:1 / 10.22:1**, respectively. ANSI slot 8 contrasts are **1.91:1 / 3.24:1**:
familiar terminal colors cannot promise readable arbitrary output. Deriving app
secondary text from ANSI 8 would fail even for an ordinary dark theme.

Probe source SHA-256: TokyoNight
`40836def07238c031f7661ece8c4c2b6fa925e7f8ac2c79f720844aac27340d8`;
Gruvbox Light `79bcb924ea79bfb563b729ee57184e729deb7648175bae97a09aedde7bfb5a80`.
These are research examples, not selected bundled themes.

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
selection and imported color records. Built-ins are code-owned; imports copy
normalized RGB data and a display name, not a path to reread. Derive application
roles centrally rather than persisting them a second time. Preference scope is
local to Desktop and shared by all windows, not Task/repository-specific.

Replace Desktop's string selection with System / built-in ID / imported ID.
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

Use **two built-ins plus imports**, with System selecting between them. Keep
recognizable cream Light and slate Dark app palettes and typography. Extend each
into coherent terminal/pane colors: cream terminal for Light and slate terminal
for Dark. The accepted design changes the stock charcoal terminal; exact colors
remain subject to native demo review. A Classic preset preserving the old pairing
was considered but is outside the accepted two-preset scope. No named third-party catalogue, theme
browser, sync or material effects.

### Semantic mapping and readability

`LoopflowPalette` owns structure, text, selection, focus, links, actions and complete
tone styles (ink/fill/stroke/text), including terminal block/search chrome.
`WorkTone` continues mapping existing state to those roles, without theme hex
literals. Terminal ANSI indices remain colors, never activity enums.

For imports, preserve terminal background/foreground and supplied indexed colors.
Derive app surfaces from the background hue with a restrained lightness ladder;
choose light/dark by its contrast polarity. Derive secondary text from foreground,
neutral selection from the surface/text ramp and focus from Loopflow's burgundy/
rose family. Keep Working blue, Waiting/Blocked amber, failure red and success
green. Adjust application role lightness for readability, never use ANSI red as
application failure authority. Labels and existing shapes preserve meaning when
hues are similar.

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

One local UTF-8 file, at most 256 KiB. Parse blank lines, full-line `#` comments and
`key = value` with surrounding whitespace. Accept six-digit RGB with or without
`#` for `background`, `foreground`, `cursor-color`, `cursor-text`,
`selection-background`, `selection-foreground`; accept decimal `palette = N=RGB`
for indices 0–255. Repeated entries use the final value and are reported. This is
an explicit supported subset, not a replacement for all Ghostty syntax.

Require background and foreground. Missing ANSI entries use the polarity's
Loopflow terminal defaults (standard xterm entries for 16–255); missing cursor and
selection colors derive from foreground/background and semantic selection roles.
Preview lists supplied, derived and omitted fields. Invalid recognized colors
report line/key and prevent applying the import. Named X11 colors, cell-relative
values, quoted/multiline syntax and conditional theme references are unsupported
initially, with an actionable message to supply RGB or a concrete theme file.

Non-color settings such as fonts, commands, keybindings, shader paths, opacity,
`theme` and `config-file` are listed as omitted and never forwarded or followed.
Generated Ghostty configuration contains normalized color data only. Cancel or
read/parse failure preserves the applied theme and saved imports. Names come from
the basename; independent imported IDs prevent equal names overwriting earlier
records. Imports are snapshots, not watched files.

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

## Alternatives and review judgments

| Mechanism | Decision |
|---|---|
| Separate app/terminal selectors | Reject: preserves discontinuity and competing owners. |
| Ghostty file as application source of truth | Reject: lacks semantic roles, can change non-color behavior and depends on external files for restoration. |
| Full Ghostty configuration import | Defer: includes/conditionals and their product meaning are a larger feature, not a shortcut. |
| Only curated built-ins | Simplest technically, but loses the familiar-color import moment demonstrated by two ordinary files. Prefer bounded import. |
| Persist imported colors plus derived app colors | Reject: stale duplicate definitions. Derive roles centrally. |
| Recreate terminal surfaces | Reject: violates continuity; the pinned live-update API exists. |

Wild success: Jack changes the environment while work stays put, including hidden
Tasks and unsent input; type, layout and semantics still read as Loopflow. Wild
failure: an attractive preview hides activity, switching kills a PTY, or restoration
depends on a moved source file. The resolved theme, snapshot imports and retention
checks address those failures.

Simulated review finding: replacing `TerminalPalette` alone misses the RGB shell
prompt, fixed block chrome, forced dark pane roles and non-color reload effects.
They belong in the same cut. A catalogue or theme-editor framework adds maintenance
without improving the demo; neither is proposed.

## Scope and deletion cut

Affected consumers: all Desktop roots/sheets; navigation/attention; Task/Workflow
headers/graph; pane controls/placeholders; file/diff editors; Ghostty config,
native background/search/block/header decoration and shell bootstrap. Update
`swift/README.md` and Desktop semantics in `VISUAL_DESIGN.md`. Shared color types
remain iOS-buildable; an iOS picker, website themes, Rust preferences and wire
changes are excluded.

**Delete — do not maintain:**

- Static `TerminalPalette` colors and `GhosttyManager.loopflowConfig` color
  literals; retain non-color defaults under their existing owner.
- Hardcoded semantic light/dark branches in `LoopflowPalette`/`WorkTone`, graph/diff
  literals, fixed native decoration colors and forced `.dark` pane environments.
  Cut consumers over together.
- Unused `LoopflowPalette.deepWine`; preserve Light/Dark definitions.
- Desktop's old `appearanceMode` writer, obsolete `AppearanceMode.resolvedTheme`
  path and tests exclusive to that representation after preference migration.
- `LoopflowZshBootstrap.accent` truecolor substitution; preserve custom-prompt
  detection, header markers, shell integration and TERM behavior.

Retain contrast, state-wording, shell-block and workspace/input tests, extending
them to themes. Forbidden: static/dynamic palette split, hidden terminal-only
color override, mirrored preference writes, theme-keyed `.id` on Work or terminals,
new Work authority, client restart or renderer correction implementation.

No layout redesign, downloader, shader/material system, color editor, font
preferences, execution migration, installation or release. Keep the inherited
`scratch/workflow-loading.md` parent hold intact; this Task does not supersede it.
Reconcile with LOO-462's delivered owners before coding; its concurrent changes
are not fully present in this baseline.

## Internal implementation sequence

One integrated delivery; these are internal cuts, not separate shipping units.

1. **This slice — collapse color ownership.** Introduce resolved theme data, fold
   static terminal colors and tone literals into it, and cut every app/native
   consumer over while preserving current palettes initially. Remove `deepWine`
   and obsolete constants. Focused check:
   `scripts/test_desktop.sh --filter 'WorkPaletteTests|GhosttyShellBlockTests' -Xswiftc -gnone`.
   Preserve semantic contrast and shell behavior through the new owner.
2. Implement preference/preview lifecycle, migration and chooser, then live Ghostty
   publication and retained decoration updates. Replace default-prompt RGB. Add
   the accepted coherent Light/Dark terminal values; delete the old resolver/writer.
3. Add import, role derivation, diagnostics and the composed headless scenario below.
   Cover partial/invalid/low-contrast inputs and personal Ghostty overrides.
   Integrate LOO-462's final consumers, update docs and inspect native coherence.

## Done when and unattended gate

The planned `ThemeWorkflowTests` uses disposable preferences and the production owner:
restore old Dark preference; load retained Repo/Wave/Task fixtures with file draft
and split layout; preview a light import; navigate/collapse/focus; cancel; apply a
dark import; construct a fresh owner to simulate relaunch. Assert selection, layout,
document and retained-view identities survive. Inspect production views for the
resolved roles, unchanged activity wording, selection/focus distinction and all
window roots. Compile generated config with pinned GhosttyKit and read back its
background, foreground and 256-entry palette. App, pane/native style and terminal
values must agree for preview, cancel, apply and restoration. Verify the effective
candidate reaches existing and future terminal consumers without launch/close
paths. Headless evidence covers owners/configuration, not Metal output.

Additional cases: System light/dark resolution, every built-in, light/dark imports,
red-heavy and middle-gray inputs; malformed RGB/index/UTF-8, missing mandatory
colors, unknown saved ID/version, duplicate keys/names, omitted non-color input;
all text/indicator compositions including selected failure and search; personal
Ghostty colors/conditional themes conflicting with the generated overlay. Default
text and selection pass; imported ANSI shortfalls remain visible and unchanged.
New acceptance tests run without `.requiresDisplay`.

Gate: `uv run python scripts/test.py --swift --loopflow`. The runner builds its CLI
fixture dependency, runs the headless Swift app/view suite and multiplatform
boundary check, then builds the Xcode terminal-fallback target. Expected: affected
suites pass, including `ThemeWorkflowTests`, importer tests, expanded
`WorkPaletteTests`, `GhosttyShellBlockTests` and retained navigation tests.
Unavailable host checks belong to capable CI; display suites remain explicitly
skipped, never counted as headless passes or gate prerequisites.

Native demo/review separately verifies active/hidden Ghostty surfaces, native
controls/titlebars, actual focus, provider input, scrollback, terminal output and
coherence across the navigation/pane matrix. Success means Jack can apply/revert
his chosen environment and continue the same work. Palette tests alone establish
neither that experience nor subjective approval.

Check (2026-10-15): `uv run python` RGB/import/contrast probe on the two linked theme files — PASS for the subset and default/selection text; TokyoNight ANSI 8 fails 3:1 as recorded above. Design contract/link/whitespace checks pass; `lf context` is within token/byte targets. Source/API inspection only; app builds and native rendering remain implementation/gate/demo checks.
