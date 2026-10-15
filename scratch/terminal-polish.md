# Terminal feedback and color — LOO-487

Jack Heart accepted implementation for hands-on review on October 15: “seems
fine. i think i need to try it to know”. Visual acceptance and landing remain
pending. Preserve the stack on LOO-462/#1531 and land the parent first; its
separate Workflow-loading hold remains live. Delivered parent owners were
integrated with `lf sync --manual` at `213136cd79` (parent `dc402dc833`).
The subsequent parent sync completed at `77ff8e8eb8` (parent `7d10108eaf`);
its stash restoration conflicted only on `scratch/questions.md`. Recovery kept
the parent's live questions there and this consolidated design, preserving the
implementation and published lf6 artifact. No implementation or publication was
replayed; the recovery stash `8a5e82290558d62211030cb19d8a88a5c4ba3e46`
remains available as evidence.
Recovery preserved all 15 implementation/artifact/test files with the parent
merge and cleared index conflicts. October 15 realignment inspected the merged
owners: nested Work layouts retain each workspace's surface pool; copy notices
still route by surface incarnation; pane focus still uses Reduce Motion-aware
`DesignAnimation.standard`. No bounded implementation mismatch was found.
The earlier lf6 test results precede this parent merge; they remain component
evidence, not a completed gate on the merged workspace.

The complete accepted design, rejected alternatives and implementation/review
evidence are preserved at `8486ff005d:scratch/terminal-polish.md` and
`8486ff005d:scratch/terminal-review.md`. Earlier parent investigations are at
`8486ff005d:scratch/workflow-loading.md`; they select no work here.

## Demo handoff

Jack Heart's demo revisions: Command-T opens and focuses a new terminal in a
vertical split first, alternating horizontal/vertical as panes are added. The
Codex/Shell header becomes a content menu. The main Session offers Claude,
Codex and OpenCode provider switching; Shell, Files and lf process open as
companion splits using the same rule, keeping the conversation in place. Reset
starts a fresh primary conversation/process with current Work instructions and
less conversation context; retain the previous conversation's history. Existing
Loopflow session controls own switching and reset. Copy's cell underlining is
too much: keep the header confirmation and remove that visual cue. Jack also
requested a subagent to investigate and improve copy/hotkey/action interoperability
with those TUIs; findings live in `scratch/tui-actions.md`.

The interactive pursue runner completed implementation, compression, sync and
realignment, then failed before launching its routing agent: response schemas
require headless mode. The operator interrupted the redundant retry and inspected
the settled steps. Publication is recovered directly; no implementation, artifact
upload or successful edge is replayed. The PID-less router record never acquired
an OS process. Broader gate and native judgment remain with ship and demo.

A separate local bundle is prepared at
`.lf/tmp/terminal-feedback/Loopflow Terminal Demo.app`, with bundle identifier
`com.loopflow.mac.demo.loo487` and no URL-scheme registration. It uses the current
Swift app and validation-only bundled CLI; it does not replace Loopflow Dev.
Keep this checkout's SwiftPM build products for its resource-bundle fallback.
Native automation is unavailable (Sky native pipe startup failed), so no visual
acceptance is claimed. Proof and build logs are under `.lf/tmp/terminal-feedback/`.

Check: current merged-source Swift app build, bundled control-tool build,
ad-hoc bundle signature verification and bundled `lf repo identity --json` PASS.
Jack Heart's hands-on review and paired native performance measurements remain.

Revision demo: `.lf/tmp/terminal-feedback/Loopflow Terminal Demo 2.app` contains
lf7 and the pane menu, Command-T and Reset changes, with a separate bundle identity
so the first demo's live terminals are not replaced. Keep SwiftPM build products.
Review caught two state issues before delivery: provider preparation now returns
the fresh workspace identity, and late reset replies update only their own Work's
selection. Changing a pane occurrence clears its old copy/title notice.
Check: `scripts/test_desktop.sh --filter 'MultiplexerStoreTests|SessionsStoreTests|DesktopPaneControlTests|SessionChromeProofTests|TerminalFeedbackTests|TerminalColorTests|TerminalPolishTests|GhosttyShellBlockTests' -Xswiftc -gnone --jobs 4` PASS (82 tests in eight suites; optional native diagnostics and separately run Metal check skipped); platform boundary check and revised demo signature PASS. Log: `/tmp/loo487-demo-revision-tests.log`. The first build was invalidated by concurrent source edits, then rerun against stable lf7 sources. Native agent switching/reset was not performed on Jack Heart's Sessions.

Demo feedback: Jack Heart reports seeing “Copied” inside Codex after dragging
text and pressing Command-C. This does not yet establish terminal-level selection
or a routing defect. The host consumes Command-C only when Ghostty supplies a
text/block selection; program clipboard writes use the separate callback without
the host's cell cue. Ghostty routes ordinary drags to programs with mouse reporting;
Shift-drag normally selects at the terminal layer. Verify the gesture in Loopflow
Terminal Demo and the location of the confirmation before changing copy ownership.

## Retained design

Jack wants calm confirmation for Copy, Find, focus and context return, with
legible program-selected colors and intact terminal text, input, drafts,
selection, scrolling and retained surfaces. This establishes no external-project
progress or sustained-use KR. October 7's retired numeric/soak acceptance stays
retired; compare costs without inventing a latency threshold.

One renderer owns cell appearance. Screen owns selection, blocks, search and
source colors; Ghostty composes shell fills before glyph contrast. Swift owns
native focus, gutter/header chrome and the parent's existing Copy/Find UI.
Program status retains its existing Rust/Swift owners; unknown activity never
animates as Working. There is no new SQLite record, DTO, status inference,
selection mirror, animation loop or theme system. iOS/external hosts are unchanged.

| Action | Presentation | Reduce Motion |
|---|---|---|
| Select/drag | Immediate native selection | Same |
| Successful Copy | Brief header notice and accessibility announcement; no cell decoration | Same |
| Next/previous Find | 160 ms edge on the newly selected match | Persistent active-match styling |
| Focus/context return | Existing focus outline and `DesignAnimation.standard` | Immediate outline |
| Reported state change | Existing semantic label/color | Immediate label/color |

Copy reads bytes and an opaque surface-local receipt under one renderer lock;
only pasteboard success confirms it. The receipt preserves clear-on-copy behavior
without scheduling a Copy animation. Selection replacement, reset, reflow,
viewport changes, hiding and destruction invalidate stale receipts. Find keeps
its finite cue; hidden panes never replay it. OSC/custom clipboard writes do
not claim visible cells were copied. Selection precedes search; glyphs and copied
bytes remain unchanged. Synchronized output cancels edges without thawing text.
Reduce Motion changes reach retained surfaces immediately.

Copy notices carry the existing surface-incarnation UUID. Each window checks
its retained pool, rejecting another window's notice or late dismissal from a
replaced surface. Find teardown closes only an existing bar, avoiding lazy UI
construction during deallocation. Failure backgrounds remain red even when
selected; the selected gutter remains neutral. Find feedback wakeups recompute
the absolute deadline after unrelated shader draws.

`minimum-contrast = 3` loads before user defaults; `1` bypasses correction exactly.
Reopen the app after configuration changes. Already-passing colors remain intact.
For failing foregrounds, interpolate toward black or white and choose the
feasible correction with the smallest linear-RGB displacement, preserving channel
ordering and premultiplied alpha.
Metal uses Display P3 luminance; GLSL uses sRGB. Unattainable ratios use the best
endpoint without making faint text opaque. This promises neither exact hue nor
separation of arbitrarily similar colors. Concealment, graphics exemptions,
cursor overrides, emoji and images retain their existing behavior. Transparent
backdrops, background images and custom postprocessing have no unconditional
contrast guarantee. User palette, selection and search colors remain authoritative.

Configuration alone collapsed semantic colors under upstream's black/white-only
correction. Swift cell overlays duplicated selection/geometry and tinted glyphs;
postprocessing lacked semantic masks and success events. Native renderer feedback
uses existing timer scheduling with viewport-bounded storage instead.

## Delete — do not maintain

Completed: Swift cell fills (`GhosttyCommandBlockStyle.fill` and its exclusive
assertions), both black/white-only shader implementations, and `TerminalCopyNotice`.
The remaining single-color `GhosttyCommandBlockStyle` wrapper is now a direct
`ghosttyCommandBlockAccent` function. Clipboard decoding goes straight from
Ghostty's bytes to String, without an intermediate Data copy or duplicate
empty-text check. Preserve block selection/status, gutter/header geometry, hover,
copy extraction, bounded reads, search counts and retained-surface tests.
No remaining predecessor path or exclusive fixture needs deletion.

Compression also removed accidental eight-component foreground test colors:
only `CellBg` grew; `CellText.color` remains RGBA. `ghostty-build` now includes
`renderer.cell` tests. Metal cases produce corrected/reference rows from one
input each; the harness no longer labels its own bypass as a production graphics
exemption. The red/green assertion selects colors by value instead of magic
array offsets. Benchmark baselines link their existing manifest instead of
recursively embedding all earlier manifests.

## Remaining delivery evidence

One implementation/PR. No production installation, provider takeover, planning
migration, broader Workflow repair, cursor trails or notification redesign.
Ghostty stays pinned to `a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0`.
The current patch is published and pinned as `GhosttyKit-a60e9e2-lf7.xcframework.zip`;
public download SHA-256 is `32e1d35ca909f67191e0d85afba8b52eb93f95a37513efbbb75f2caf195b44ed`.
Earlier lf4/lf5/lf6 uploads remain immutable. Jack Heart's October 6 authorization
covers future artifacts through the patch README workflow.

Gate owns affected suites and acceptance once; reuse identical content/commands:

| Command | Evidence |
|---|---|
| `uv run python scripts/loopflow-dev.py ghostty-build` | Patch/parser/ABI and renderer-cell tests, both shader backends, universal ReleaseFast framework |
| `scripts/test_desktop.sh --filter 'TerminalColorTests\|TerminalFeedbackTests\|TerminalPolishTests\|GhosttyShellBlockTests\|WorkPaletteTests\|WorktreeWorkspaceTests\|DesktopHeadlessTests' -Xswiftc -gnone` | Color, host models, retained state and shell output without WindowServer; build current fixture lf where needed |
| `uv run python scripts/test.py --loopflow` | Xcode fallback app/tests compile; no renderer claim |
| `uv run python scripts/check_swift_multiplatform_boundaries.py` | Platform separation |

Production Metal functions and background fragments run offscreen; GLSL has
compile validation using the renderer's include-expanded sources, not rendered
color parity evidence. A missing Metal device defers the shader check to capable
macOS CI; the build script fails rather than automatically dispatching that work.
These checks do not prove full glyph composition, graphics-exemption routing or
source-to-presented-pixel behavior. The existing Zig cases prove receipt
invalidation, wrapped/wide/combining text preservation, finite deadlines and block
style precedence; Swift proves copy-message/count models and incarnation routing.
Failed pasteboard writes, the composed search lifecycle, imported palettes and
mounted draft/focus/scroll retention still need behavioral evidence.
Gate must distinguish covered assertions from still-unproved acceptance:

- Exact UTF-8/line endings, wrapped/wide/combining text and blocks; no cue on
  failed/empty/stale reads or failed pasteboard writes; repeated/clear-on-copy.
- Find pending/empty/selected/closed states, selection precedence, focus restored
  only when Find owned it; resize/reset/hidden output and switching preserve
  state without stale flashes, duplicate surfaces or stolen input.
- ANSI 16/256 and truecolor, imported light/dark palettes, OSC changes,
  inverse/selection/search, faint/concealed and exempt glyphs. Bypass/passing
  colors unchanged, red/green distinct, unattainable cases classified.
- Feedback stops at its deadline with no further cue redraws. No typing-path
  polling, scrollback copy, subprocess or GPU synchronization. Measure existing
  cursor/Kitty/custom-shader scheduling separately.

Demo/review: use two retained panes with configured Codex/Claude, a shell, diffs
and a colorful utility. Copy a multiline error, Find and navigate it, return to
waiting work, resize/switch and confirm unsent drafts, selection and scrolling.
Repeat with difficult colors, imported light palettes, original-color mode and
Reduce Motion; Jack judges whether cues are helpful and calm.

`scripts/desktop_performance.py terminal-feedback` prepares the contained fixture;
`--mounted` collects optional first/warm draw-call diagnostics and CPU/RSS logs.
Compare identical optimized baseline/candidate scenarios: idle, typing, copy
bursts, Find and retained returns, recording source/artifact/input/configuration,
scale/viewport, font and pane count. Preserve failed attempts. Native capture
must separately establish input-to-visible-character, onset/expiry and GPU/frame
cost. Draw-call diagnostics and replayed output prove neither presented pixels
nor provider readiness. No measured improvement or visual acceptance is claimed.

Check: reconciliation `git diff --check` and `git diff --cached --check` PASS; `lf context` PASS (memory and scratch within both limits; no child Product memories present); no implementation changes or test reruns. Retained pre-sync evidence: `uv run python scripts/loopflow-dev.py ghostty-build` with Zig 0.16 on PATH PASS (283 Zig tests, 4 skipped; 324 Metal cases; GLSL; universal ReleaseFast; lf6 public checksum verified); `scripts/test_desktop.sh --filter 'TerminalColorTests|TerminalFeedbackTests|TerminalPolishTests|GhosttyShellBlockTests' -Xswiftc -gnone --jobs 4` PASS against public lf6 (10 passed, native diagnostic and already-run Metal subprocess skipped); artifact pytest, paired fixture preparation, Ruff and diff check PASS. Initial build selected installed Zig 0.15.2 and failed; rerunning with the existing 0.16 toolchain resolved it. Gate owns broader acceptance; demo/review owns native judgment and paired performance. Logs: `/tmp/loo487-compress-{ghostty,lf6}.log`.
