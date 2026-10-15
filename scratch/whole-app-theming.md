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

The October 15 source-only TokyoNight/Gruvbox Light probe established that ordinary
Ghostty files fit the bounded RGB subset. Default-text contrast was 10.59:1/10.22:1
and selection text 5.64:1/10.22:1, but ANSI 8 was only 1.91:1/3.24:1. Therefore
application secondary text derives independently; imported ANSI colors retain
warnings. Exact source links and hashes are in the archived survey. These examples
were research inputs, never selected bundled themes.

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
neutral file selection from the surface/text ramp and focus from Loopflow's
burgundy/rose family. Work destinations use the parent's accepted Ink on Paper
fill, adjusted for contrast, with independently readable primary/secondary text. Keep Working blue, Waiting/Blocked amber, failure red and success
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

**Completed deletion cut:**

- Static `TerminalPalette` colors and `GhosttyManager.loopflowConfig` color
  literals removed; non-color defaults retain their existing owner.
- Hardcoded semantic light/dark branches in `LoopflowPalette`/`WorkTone`, graph/diff
  literals, fixed native decoration colors and forced `.dark` pane environments.
  Consumers now use resolved roles.
- Unused `LoopflowPalette.deepWine` removed; Light/Dark definitions remain.
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

No layout redesign, downloader, shader/material system, color editor, font
preferences, execution migration, installation or release. Keep the inherited
`scratch/workflow-loading.md` parent hold intact; this Task does not supersede it.
LOO-462's `7d10108eaf` integration adds Jack Heart's accepted Ink on Paper
selection, drill-in navigation and outer Work layouts. Those owners remain intact.
WorkStyle's fixed selected-color extension is removed; selected fill and text
resolve through LoopflowPalette. ThemeWorkflowTests now uses the sidebar's
production collapse/restore controls instead of the removed WorkBreadcrumbBar.

## Implementation and remaining review

Implemented October 15, integrated through LOO-462 `7d10108eaf`: theme, import,
persistence and live preview. The existing registry owns native views;
hidden backing layers, shell blocks, search and copy feedback observe changes.

The deletion cut is complete: no static terminal color owner, forced dark pane
palette, fixed status literals, deepWine, old Desktop preference writer or old
appearance resolver remains. Portfolio rows and telemetry also consume the theme.
Personal theme selectors clear before Ghostty finalization; non-color settings
remain in the frozen baseline. The current and applied compiled candidates are
retained so Cancel can reuse the validated configuration. The cache owns those
configurations; initialization uses a local handle rather than a second stored alias.

Review fixes: mid-gray imports exposed unreadable chip fills, so optional chip and
shell-block tints reduce when they would break text contrast. Corrupt saved bytes
remain intact with a recoverable notice. Invalid imports cannot publish or persist;
non-color input never reaches Ghostty. File and Portfolio selection keep a neutral edge; Work destination selection
uses Ink on Paper with contrast-adjusted fill and text from the shared palette. The parent sync retained its pane islands and search.

Implementation follow-up, October 15: headless `ThemeWorkflowTests` now drives the
production sidebar toggle, Wave/Task rows, attention entry and Back with retained
Repo/Wave/Task owners. Both built-ins and light/dark imports exercise preview,
Cancel, Apply and preference restoration. Session/shell native-view identities,
file draft/selection, split layout and model focus survive. No provider or Ghostty
surface is launched.
The new outer Work layout is covered for preview/Apply with adjacent Task/Wave
workspaces and hide/return; inner pane layouts survive preview/Cancel/Apply.
`AppAppearanceTests` inspects the shared modifier and System light/dark resolution.
Constructing seven unrelated roots repeated identical outer-modifier assertions;
the focused test preserves those assertions without implying window-wiring proof.
`WorkPaletteTests` checks status wording, unknown activity, selection and focus
combinations across the four appearances. Environment modifier inspection is not
mounted SwiftUI propagation or native rendering proof.

Dim diagnostics now estimate default foreground and ANSI 0–15 at Ghostty's default
50% faint opacity, retaining original colors and permitting Apply. The estimate is
labeled: personal faint-opacity and per-cell backgrounds can differ. It uses the
pinned renderer's rounded alpha (128/255), without changing terminal configuration.

**Readback extension, October 15:** `0005-terminal-color-readback.patch` extends
Ghostty's existing C getter with RGB `TerminalColor` conversion into
`ghostty_config_color_s`. Unset and cell-relative values return false without
writing; no union layout crosses the boundary. Production validates all ten named
colors and all 256 indexed entries after finalization. Tests replace the lf3
limitation assertion with compiled readback against conflicting personal colors.
Rejection cases cover all ten named keys, indexed entry 255, and unset/cell-relative
cursor colors. The patched universal framework and headless Desktop tests pass. Its version is
`a60e9e2-lf3-theme-readback1`; no app release/install/restart occurred.
This closes the earlier readback gap; Product memory now reflects the fixed-RGB
contract and its native evidence limits. Product has no child memory directories.

Simulated review: generate and validate managed Ghostty colors from one mapping;
keep expected values independent in tests. The chooser needs no binding wrapper.
The C getter preserves failure semantics without a new ABI layout. Native rendering
and atomic updates remain outside compiled configuration proof.

Source inspection on October 15 finds the shared AppAppearance on workspace,
Portfolio, Open Work, Appearance and Telemetry scenes. The headless modifier test
does not mount those scenes. Gate owns affected suites, iOS boundary and Xcode
fallback build. Native demo owns multi-window propagation, provider input, actual
focus, scrollback, controls/titlebars and visual coherence.
The inherited navigator note's two WorkCacheTests failures remain gate findings.
Exact palette/ladder values remain demo judgments.

## Done when and unattended gate

Required acceptance (headless owners/views covered; native behavior remains at demo): use disposable
preferences and the production owner to restore old Dark preference; load
retained Repo/Wave/Task fixtures with file draft
and split layout; preview a light import; navigate/collapse/focus; cancel; apply a
dark import; construct a fresh owner to simulate relaunch. Assert selection, layout,
document and retained-view identities survive. Inspect production views for the
resolved roles, unchanged activity wording and selection/focus distinction;
scene wiring is source-inspected and mounted propagation remains at native review.
Compile generated config with pinned GhosttyKit and read back background,
foreground, cursor, selection, search and 256 indexed colors through the patched C getter. App, pane/native style and terminal
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

Check (2026-10-15, reused at realign; no code changes): `scripts/test_desktop.sh --filter 'ThemeTests|ThemeWorkflowTests' -Xswiftc -gnone` — app/test build and 10 tests PASS (`/tmp/loo488-theme-compress-tests.log`); patched framework build remains applicable; broader gate and native demo remain.

Check (2026-10-15, publication): `scripts/test_desktop.sh --filter 'AppAppearanceTests|WorkPaletteTests|GhosttyShellBlockTests|GhosttyTerminalInputTests|WaveLensTests' -Xswiftc -gnone` — build and headless checks PASS (`/tmp/loo488-publication-tests.log`); optional display diagnostics skipped; broader gate and native demo remain.

Publication review: the Task base is `7d10108eaf`; the diff preserves the parent's navigation/pane owners and holds. The normalized overlay and compiled readback share one color mapping, and imports cannot inject non-color configuration. No further publication blocker found. Native rendering/input continuity and the inherited WorkCacheTests findings remain explicit review/gate limits, not claims of this headless proof.
