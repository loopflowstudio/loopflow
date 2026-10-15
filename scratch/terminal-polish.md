# Terminal feedback and color — LOO-487

Status: accepted for implementation and demo, 2026-10-15. Jack Heart reviewed
the proposal: “seems fine. i think i need to try it to know”. Build the proposed
3:1 contrast default, original-color override and brief feedback for hands-on
review. Visual acceptance and landing remain pending that demo. Preserve the
LOO-462 stack. Implementation is complete; native visual acceptance and measured
performance remain with gate/demo. The delivered LOO-462 owner was integrated
through `lf sync --manual` at merge `213136cd79` (parent `dc402dc833`).

## Problem and intended experience

Jack Heart wants terminal workspaces to sustain long coding sessions: copying
text, finding an error, moving between panes and returning to waiting work should
give clear, calm feedback. Program-selected colors must remain legible without
flattening diff, syntax, ANSI or status distinctions. Preserve terminal fidelity,
fast input, retained clients, drafts, selection and scrolling.

This serves Product's work-first workspace and sustained-use chapter outcomes.
It establishes no external-project progress or sustained-use KR by itself.
October 7's retired numeric/soak acceptance stays retired; compare measured costs
without inventing an input-latency target.

## Demo

In the development app, open two retained terminal panes containing an agent CLI
and a shell. Select and copy a multiline error: the copied cells briefly acquire
a neutral edge and the existing copy notice confirms the line count. Clipboard
bytes are unchanged. Find the error, step through matches, then leave and return
to a waiting Task: the current match and actual keyboard destination are clear,
with the unsent draft, selection and scroll position intact.

Repeat with a colored diff and a colorful terminal utility, dark and imported
light palettes, difficult truecolor pairs, and Reduce Motion. Red and green text
remain distinguishable while low-contrast text becomes readable. Set
`minimum-contrast = 1` in the existing Ghostty configuration and reopen the dev
app to compare original colors. No theme chooser is added.

Provide a new contained `terminal-feedback` scenario in the existing
`scripts/desktop_performance.py` runner for repeatable fixture output and paired
measurements; its mounted mode is optional demo evidence, never a headless gate
prerequisite. Configured Codex/Claude interaction is a separate native review,
not something a replayed transcript proves.

## Evidence and boundaries resolved

- **Current checkout:** `c9bc78904b52fdceff46959ce011a394a28fb706`.
  `GhosttyTerminalView.swift` retains one `GhosttyMetalView`/surface per terminal
  in the window's pool. `Screen` owns mutually exclusive text/block selection
  through `0001-command-blocks.patch`; Swift holds hover and derived block
  geometry. Preserve these lifetimes and ownership.
- **Parent dependency:** Jack Heart confirmed on October 15 that LOO-487 must stay stacked on LOO-462.
  The Task records parent PR #1531 (`pr_48999a99e95141fd8ecb3cc7c7b5aa49`);
  integrate subsequent parent changes through Loopflow's stack sync and land it first.
  LOO-462 owns rounded panes, program headers, navigation, ordinary copy
  confirmation and unobstructed Find. These are not all in this
  checkout. A read-only inspection on October 15 found uncommitted parent work
  over `52ee931af5`: `TerminalSearchBar` is a sibling of the terminal mount,
  `TerminalSearchResults` handles pending/empty/selected results, and
  `copySelection()` calls `showCopyConfirmation()` only after pasteboard success.
  The notice currently sits over terminal cells. Integrate that parent's delivered
  owner before the Swift slice; do not copy its working files or build parallel
  UI. These observations are implementation evidence, not a stable API or proof
  that the parent has landed. The inherited `scratch/workflow-loading.md` records
  its unresolved landing hold; this Task does not operate that work.
- **Pinned renderer:** `swift/Package.swift` selects Ghostty
  `a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0`, artifact `lf3`.
  The downloaded source confirms `minimum-contrast` defaults to **1**, and both
  Metal and GLSL `contrasted_color` choose pure black/white on failure. Turning
  that setting up alone can turn two low-contrast semantic colors into one.
- **Color is available before pixels:** `src/renderer/generic.zig` resolves
  SGR/inverse, selection and search styles; text selection precedes search.
  The text vertex shaders receive foreground and resolved background, before
  glyph coverage. Source terminal colors remain separate from rendered colors.
  `cell.zig:noMinContrast` exempts box/block/Powerline graphics; concealed text
  skips glyph rendering. Preserve those rules and emoji/image behavior.
- **Find already exists in the engine:** `Surface.zig` owns asynchronous search;
  bindings `search:…`, `navigate_search:next/previous`, `end_search` and the four
  search action callbacks are exported. Renderer messages already distinguish
  ordinary and selected matches. No Swift scrollback extraction/search index is
  needed. The parent implements the missing host UI.
- **Custom postprocessing is the wrong interface:** the pinned
  `shadertoy_prefix.glsl` exposes pixels, cursor/focus and palette uniforms, but
  no selection ranges, match identity or successful-copy event. Recovering cells
  from pixels would recreate the original problem. Its animation mode can keep
  focused or all panes rendering. Do not install a whole-screen effect shader.
- **Existing scheduling is sufficient:** `generic.zig:animationWake()` and
  `renderer/Thread.zig` combine custom-shader and Kitty deadlines. Add bounded
  feedback deadlines there, with visibility suppression. The Swift display link
  already calls draw; do not add a second animation loop or increase its rate.
- **Composition matters:** `renderCommandBlocks()` currently puts translucent
  failure/selection/hover CALayer fills *over* terminal glyphs. Shader contrast
  cannot account for that later tint. Move only those cell-area fills into the
  existing Ghostty background pass, retaining Swift gutter/header/separator
  chrome and the terminal's real exit status. Do not “fix” the source palette.
- **Build boundary:** SwiftPM links GhosttyKit; Xcode currently compiles the
  terminal-disabled path. A fallback build is not renderer proof. The existing
  `ghostty-build` script applies local patches, runs Zig tests and builds the
  dual-architecture framework. `GhosttyKitPatches/README.md` records Jack Heart's
  October 6 authorization for future artifact updates through that workflow.
  This design step neither publishes an artifact nor changes that authorization.

Primary references (inspected October 15): [pinned renderer](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/renderer/generic.zig),
[pinned Metal shader](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/renderer/shaders/shaders.metal),
[pinned configuration](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/config/Config.zig),
[Ghostty option reference](https://ghostty.org/docs/config/reference#minimum-contrast).
The pinned source, not newer documentation, controls API availability.

## Chosen approach

### One renderer owns cell appearance

Extend the maintained patches with terminal-feedback support; keep the upstream
revision fixed for this change. The terminal remains the source of selection,
search matches, SGR colors and command exit status. The renderer derives cell
appearance, correction and temporary feedback. Swift owns native focus and the
parent's copy/search UI. Existing Rust/Swift status observations remain the only
source of Working, Waiting, Blocked and Error. No new SQLite record, DTO, status
inference or CLI control API is needed; iOS and external terminal hosts are unchanged.

Move block fill composition into `generic.zig` before text contrast. Feed existing
shell hover intent through a small surface setter; resolve blocks from Screen's
tracked pins, not Swift's row snapshots. Apply this styling only to shell surfaces.
Keep neutral selection distinct from the red failure fill; header/gutter geometry
continues to derive from the existing block query. Resolve neutral fill against
the terminal's effective palette, including imported light configurations.

Add presentation-only feedback state to the existing surface/render path: current
selection revision, current match identity and bounded effect deadline. Never
persist it or use it to control a Session. Bound storage to the viewport/existing
selection, not copied text length or scrollback size.

Copy uses the parent's successful-copy path. Extend the existing selection read
with an opaque surface-local revision receipt captured under the same renderer
lock as the bytes; after pasteboard success, confirm that receipt to the surface.
Only a still-current receipt may illuminate its cells. A changed selection,
screen reset, surface destruction, reflow or viewport change cancels the temporary
cue instead of highlighting unrelated text. Do not retain raw surface pointers in
asynchronous callbacks. No cue for failed/empty reads or failed pasteboard writes.
Programmatic OSC clipboard writes and non-selection custom copy actions retain
behavior but do not claim that visible cells were copied.

The parent notice and accessibility announcement survive. Move its visual placement
out of the cell rectangle using the existing mount/pane chrome, retaining one
notice owner; no second toast, search bar or clipboard writer.

### Feedback policy (defaults for the demo)

| Action | Presentation | Reduced motion |
|---|---|---|
| Select/drag | Immediate native selection; no delayed selection or animated scrolling | Same selection |
| Successful Copy | Neutral edge on the copied cell range, one 160 ms ease-out; existing textual confirmation | Static edge until the same deadline, same notice/announcement |
| Next/previous Find match | One 160 ms edge emphasis on the newly selected match; normal active-match styling remains | Immediate persistent active-match style |
| Pane focus/context return | Reuse `DesignAnimation.standard` for the existing focus outline; no pixel motion or fade of terminal contents | Immediate outline |
| Reported state change | Existing semantic label/color changes; at most the existing brief transition | Immediate label/color |

Copy is neutral, not green success or red failure. Edges never recolor glyphs or
capture pointer events. Current text selection retains precedence over search;
copy emphasis cannot conceal either. Repeated actions replace the current deadline,
not queue pulses. Hidden panes cancel transient cues, retain semantic state and
never replay old effects on return. Reduce Motion changes propagate to retained
surfaces immediately, including an effect already running. Unknown activity stays
unlabelled; selection/focus is not evidence of Working. Preserve the parent's
native-responder test rather than treating pane selection as keyboard focus.

### Contrast without erasing the palette

Set `minimum-contrast = 3` in `GhosttyManager.loopflowConfig`, which loads
**before user defaults**. Keep `1` as the exact correction bypass. Existing user
values win; preserve their palette, search and selection colors. This is a
rendering default, not an accessibility certification or a new theme definition.
Configuration is presently initialized once: document reopening the app for an
edited setting; do not recreate retained surfaces to apply it.

Replace the existing black/white-only `contrasted_color` in Metal and GLSL in
place. For an opaque glyph below the requested ratio R, compute the two feasible
target luminances relative to the resolved background B:

- darker: `(L(B) + .05) / R - .05`;
- lighter: `R * (L(B) + .05) - .05`.

Interpolate the foreground toward black or white only far enough to reach each
feasible target; choose the candidate with the smaller squared linear-RGB change.
Keep already-passing colors exactly unchanged. This preserves channel ordering
and more chromatic information than replacing the entire color, without promising
perceptually constant hue or separation of arbitrarily similar input colors.
If neither target is feasible at a user-selected high ratio, use the endpoint
with the best attainable contrast and report that limit in the fixture results.

Perform calculations in the shader's actual linear color space: Metal's converted
Display P3 and OpenGL's sRGB need their respective luminance coefficients. Preserve
premultiplied alpha; evaluate faint text over the actual background and solve within
its original alpha's attainable range. Never make faint text opaque just to pass.
Keep glyph coverage, concealment, cursor overrides, graphics exemptions and images
unchanged. Transparency over unknown desktop content, background images and
user-authored postprocessing cannot receive an unconditional contrast guarantee;
show these as explicit limits, not passing opaque-text cases.

A CPU numerical probe on October 15 supports the mechanism, not shader parity:
against `#24211F`, dark red `#442222` moves from 1.141:1 to approximately
`#746868`, and dark green `#224422` from 1.461:1 to approximately `#616F61`;
both unquantized results reach 3:1. Upstream turns both white. Against `#FAF8F5`,
matching foreground becomes approximately `#91908E`; `#888888` already passes and
is unchanged. A mid-gray background also has a feasible darkening route. Allow
quantization tolerance in pixel tests; the analytical calculation alone proves
neither P3 correctness nor rendered legibility.

## Alternatives and failure analysis

- **Configuration only:** cheap and supported, but black/white collapse fails the
  semantic-color requirement and cannot acknowledge successful Copy. Rejected.
- **Swift overlays for every cell:** avoids a binary change but duplicates range,
  wrapping and scroll ownership. The prior command-block selection duplication
  already failed; color contrast would still see the wrong composited background.
  Rejected for cell effects; retain Swift's pane and notice responsibilities.
- **Custom GLSL postprocessing:** blocked by missing semantic masks/events and
  affects glyph pixels globally. Adding those interfaces is as much work as the
  native renderer route with less control. Rejected.
- **Native cell rendering:** selected. One artifact update and paired Metal/GLSL
  maintenance are the real cost. No new dependency or shader catalogue.

Wild success is confidence without attention tax: Jack copies once, finds the
right error, returns to the right draft and can still read what red/green means.
Wild failure is a calm-looking screenshot that drops selection on resize, confirms
an unsuccessful copy, brightens concealed text or keeps every retained pane busy.
Those are behavioral tests below, not visual polish deferred indefinitely.

## Delivery and internal slices

One coherent implementation/PR; slices are construction order, not independent
compatibility releases. Preserve the recorded stack on LOO-462/#1531 and integrate
parent changes through Loopflow's stack sync. Jack Heart's October 15 confirmation
is retained in `scratch/stack-direction.md`; it grants no parent landing approval.
Land the stacked parent first. No live provider takeover,
production installation, planning migration, theme catalogue, cursor trails,
continuous glow, new notification policy or broader Workflow-loading repair.

1. **This slice — correct composition and color.** Remove Swift's over-glyph
   block fills while moving the same selected/failed/hover behavior into Ghostty.
   Replace contrast in both shader backends and set the overrideable default.
   Add headless `TerminalColorTests` and patch tests named `terminal contrast`;
   preserve block reflow/selection and original-color bypass. Focused proof:
   renderer color vectors plus composed block-background cases; build the patched
   framework and changed Swift app. No requirement for a display.
2. **Action feedback and parent cutover.** Integrate LOO-462's actual delivered
   `TerminalSearchBar`/copy owner, add selection receipts and finite renderer
   effects, and feed Reduce Motion to retained surfaces. Use existing match
   messages and native focus events. Extend parent `TerminalPolishTests`; add
   `TerminalFeedbackTests` for stale receipts, cancellation and lifetime. Preserve
   exact clipboard bytes, notification text, keyboard routing and drafts.
3. **Artifact, proof and demo.** Extend `ghostty-build` filters for new behavior
   and shader verification; build a new immutable artifact version (next available
   after lf3, not an overwritten lf3). Verify public download/checksum before
   updating Package.swift. Keep build script, patch README and artifact pin in
   agreement. Update `swift/README.md` with effects, override and limits. Run gate
   once, then collect paired native measurements and review the demo.

**Delete — do not maintain (completed):** removed `GhosttyCommandBlockStyle.fill`,
`surfaceLayer.backgroundColor = style.fill.cgColor`, and their exclusive Swift
fill assertions; renderer tests now cover composition and failure precedence.
Replaced both old black/white-only shader implementations. Removed the parent's
`TerminalCopyNotice` overlay; `showCopyConfirmation` now feeds the pane strip
through surface-incarnation-scoped notices. Keep block selection/exit status,
headers, gutter markers, hover, copy extraction, bounded reads, search counts and
retained-surface tests. Do not maintain old/new rendering paths, a Swift selection
mirror, duplicated search storage or test-only production interfaces.

## Done when and gate contract

The test/scenario names below now exist. Gate owns the complete affected-suite
run; implementation ran focused behavior and build checks. The source-to-consumer scenario feeds ANSI/truecolor,
inverse/faint/conceal, a failed shell block and repeated error text into the real
terminal model; selects/copies, confirms the receipt, navigates search, advances
the effect clock and resizes. Assert original text/style storage and copied bytes,
resolved cell/background output, final shader pixels and the host's derived
notice/count/focus behavior. Terminal engine and AppKit boundary proofs compose;
headless host tests do not pretend to mount a native provider.

| Gate command | Required result |
|---|---|
| `uv run python scripts/loopflow-dev.py ghostty-build` | Existing patch/parser/ABI tests plus `terminal contrast` and `terminal feedback` pass; both shader backends compile; both framework architectures export the new receipt/feedback API |
| `scripts/test_desktop.sh --filter 'TerminalColorTests|TerminalFeedbackTests|TerminalPolishTests|GhosttyShellBlockTests|WorkPaletteTests|WorktreeWorkspaceTests|DesktopHeadlessTests' -Xswiftc -gnone` | Real color fixtures, parent UI models/views, retained state and shell output pass without WindowServer; build current fixture `lf` first where required by the suite |
| `uv run python scripts/test.py --loopflow` | Xcode fallback app and test runners compile; no claim of Ghostty rendering |
| `uv run python scripts/check_swift_multiplatform_boundaries.py` | Platform separation remains valid |

Add a headless offscreen Metal check to `TerminalColorTests`, exercising the
**production shader** into a texture for opaque, dim, selected, search, block-fill,
sRGB/P3 and exempt-glyph fixtures. It must not require a window or permission
prompt. A local `MTLCreateSystemDefaultDevice()` probe succeeded without opening
a window; this establishes device availability, not shader execution or CI parity.
If the host has no suitable device, record that check as unavailable and
route it to a capable macOS CI runner; a CPU reimplementation is not shader proof.
GLSL is compiled at pipeline creation, so a Zig/library build alone is not shader
compilation proof. Extend `ghostty-build` with offline GLSL validation using the
same include-expanded source and version prefix as `opengl/shaders.zig`, alongside
the Metal production-shader test; fail on shader errors rather than silently
skipping the second backend.

Required observable assertions:

- Copy succeeds exactly once with exact UTF-8/line endings, including wrapped and
  wide/combining text and command blocks; empty/failure/stale receipt yields no
  cell success cue. Repeated copy, clear-on-copy configuration and selection
  replacement do not illuminate the wrong range.
- Search pending, no matches, selected match and close remain distinct. Selection
  wins overlap; closing Find restores input only when Find owned it. Resize,
  switching, hidden output and reset preserve valid semantic state without stale
  match/copy flashes, duplicated surfaces or stolen focus.
- Color vectors cover 16/256-color and truecolor, imported dark/light palettes,
  OSC palette changes, inverse/selection/search and faint/concealed text. Passing
  colors and `minimum-contrast=1` are unchanged; chosen red/green fixtures remain
  distinguishable; unattainable and exempt cases are explicitly classified.
- Effects terminate at their deadline and request no further feedback redraws.
  No additional polling, full scrollback copy, subprocess or GPU synchronization
  is introduced on the typing path. Hidden/reduced-motion cases do not animate.
  Existing cursor/Kitty/user-shader schedules are measured separately.

The new `terminal-feedback` benchmark records baseline/candidate source and
artifact hashes, configuration, scale/viewport, font, fixture input and visible
pane count. Use identical optimized builds and scenarios: idle, sustained typing,
copy bursts, Find navigation and repeated retained-Task switching. Compare
input-to-visible-character, effect onset/expiry, frame/GPU time, process CPU and
RSS; include first-use and warm samples and failed attempts. Existing
`terminal_key_to_draw` ends at a draw call, **not** presented pixels or provider
readiness. Retain it as a separate diagnostic; native capture must establish the
visible endpoint. Do not claim measured improvement before these runs.

Native demo/review judges whether the cues are helpful and calm on representative
Codex/Claude screens, diffs and a colorful utility, with original-color mode and
Reduce Motion. Missing display access defers that judgment, not the headless
implementation checks. Delivery evidence must state measured costs and remaining
limits rather than inventing a latency threshold or reviving a soak requirement.

## Remaining delivery evidence

- Final artifact is published and pinned: `GhosttyKit-a60e9e2-lf5.xcframework.zip`,
  public download SHA-256 `032f84a23d685cb87be75d06222d2bb07d3b8a847e4631e27beb8799bcb8be62`.
  The earlier `lf4` upload completed before cancellation; it is not selected by
  the final manifest and is never overwritten.
- Gate: run the affected suites and automated acceptance commands above once.
  The focused checks cover receipt lifetimes, geometry/reset invalidation,
  exact wrapped/wide/combining text, selected-failure appearance, surface-local
  notice routing, finite deadline scheduling, production color functions and
  background-edge fragments. The Metal harness does not claim full glyph or
  source-to-presented-pixel proof; the mounted fixture remains a separate check.
- Demo/review: collect paired optimized native observations and presented-pixel,
  GPU/CPU/RSS evidence; judge cues on configured Codex/Claude, diffs, imported
  palettes, original-color mode and Reduce Motion. No latency improvement or
  visual acceptance is claimed. The inherited parent landing hold remains.

Review repairs are in `scratch/terminal-review.md`: both queued findings plus
absolute expiry under custom-shader redraws and safe unopened-Find teardown.
No parallel clipboard writer, search store, selection owner or animation loop
was added. No Wave memory change is selected.

Check: `zig build test -Dtest-filter='terminal contrast' -Dtest-filter='terminal feedback'` and `zig build -Doptimize=ReleaseFast -Demit-macos-app=false` PASS; `scripts/test_desktop.sh --filter 'TerminalColorTests|TerminalFeedbackTests|TerminalPolishTests|GhosttyShellBlockTests' -Xswiftc -gnone --jobs 4` with `LOOPFLOW_GHOSTTY_SOURCE` PASS against the public lf5 artifact (12 tests, one native diagnostic skipped; 324 production Metal cases and background fragments). Offline GLSL, Python artifact test, Ruff and platform-boundary checks PASS. Gate owns broader suites; demo/review owns native appearance and paired performance evidence.
