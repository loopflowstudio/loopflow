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
Codex and OpenCode provider switching. The later pane-switcher direction below
supersedes adding a companion on every header selection. Reset
starts a fresh primary conversation/process with current Work instructions and
less conversation context; retain the previous conversation's history. Existing
Loopflow session controls own switching and reset. Copy's cell underlining is
too much: keep the header confirmation and remove that visual cue. Jack also
requested a subagent to investigate and improve copy/hotkey/action interoperability
with those TUIs; findings live in `scratch/tui-actions.md`.

Jack Heart's next demo revision: new Task workspaces start Session/browser at
70/30, side by side. Header choices switch retained Task, PR, Files, Shell and
lf-process views in the companion; new shells/splits remain explicit. Hidden
shells keep running, and file drafts survive switching. Sidebar Task selection
still switches whole workspaces; selections within a Task may focus its companion.
No tab strip or permanent Task/PR navigation column. Task and PR lead the header
choices; Task overview starts selected. Remove Workflow chrome and Details from
the workspace. One Task lifecycle control reveals Complete/Pause/Abandon on
hover or click. Compact file rows, a narrower collapsible tree and no reserved
empty PR strip recover reading space. Existing layouts must not be reset.

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

Review: retained shell commands/surfaces now outlive companion switches; closing
the pane retires a hidden shell. Session selections focus a companion instead
of replacing the main conversation. Reuse an already-visible document view to
avoid mounting its native editor twice. Initial Task overview and explicit Files
entry remain distinct. Removed obsolete Details-sheet diagnostics; Workflow APIs
remain available outside the compact workspace. Native geometry, hover and live
provider behavior remain demo judgment.

Check: `scripts/test_desktop.sh --filter 'MultiplexerStoreTests|DesktopPaneControlTests|TaskFilesTests|WorkNavigationTests|TaskFlowProofTests' -Xswiftc -gnone --jobs 4` PASS (128 tests reported; five optional native cases skipped); Swift multiplatform boundary and diff checks PASS. `/tmp/loo487-pane-switch-tests-final.log`. Initial compilation found obsolete Details-sheet benchmark references; the next run exposed an obsolete removed-toolbar fixture, and the following run an accessibility-tree assertion; all were corrected without disabling current behavior checks.

## Navigation revision — implementation authorized

Jack Heart requested default-closed navigation, opened when needed. Its compact
control starts filled with the repository name as the selected item. Keyboard
controls must still open and keep today's full sidebar visible, or close it.
Jack subsequently requested building and opening this demo. This supersedes
default-open navigation; native usability remains review evidence.

Usage: click the selected-location control or press Command-K to open
a searchable chooser. Browse Repo → Wave → Task or filter the existing Work
results; selection changes the retained workspace and closes the chooser.
Escape dismisses it only while navigation owns focus, restoring the prior
responder. Control-Command-S toggles the existing resizable sidebar between
pinned-open and closed; pinned navigation survives selections. No hover opening
or pointer-exit closing. The closed control reflects the current destination,
starting at the repository. Reuse current search, selection and history owners.

The existing palette owns the transient chooser; the existing sidebar flag now
starts false and retains explicit pinned visibility. Together they represent
closed / choosing / pinned without another selection or search owner. The
chooser is a popover over unchanged pane geometry; pinned navigation reserves
sidebar width as today. Preserve per-Wave scroll, filter, remembered width,
Command-click adjacent Work, drafts and terminal identity. Empty results or a
failed read keep the chooser open without changing selection; Escape recovers.
Next proof: keyboard-only open/search/select/dismiss/pin/close, repo-first launch,
retained focus and drafts, and whether the compact control reads clearly.

Review: removed the startup sidebar flash and limited navigation shortcuts to
the owning window and its chooser. Extra modifiers and terminal Escape remain
provider input. Existing destination tests now explicitly replace their addressed
pane; selecting Changes intentionally focuses the companion. Removed a stale
assertion for the deleted worktree toolbar.
Check: navigation/destination/pane suites PASS (133 tests reported, six optional
native cases skipped), semantic palette suite PASS (4), platform boundary and
diff checks PASS. Logs: `/tmp/loo487-nav-tests-4.log`,
`/tmp/loo487-nav-palette-tests.log`. Native popover/focus checks remain unrun.
Demo 4 was signature-verified and opened (PID 97144); source/copy hashes live in
`.lf/tmp/terminal-feedback/demo4-package-proof.json`. Jack Heart's visual and
keyboard review remains at demo; earlier demos and their Sessions remain running.

### Dropdown correction

Jack Heart rejected the small plain control and flat search palette. The closed
control belongs in the Session workspace header, and opening it must drop the
previous navigator layout down from the top. Overlaying the workspace is accepted.
Pinning/expanding restores the prior docked sidebar structure; temporary navigation
does not resize panes. Both presentations use WorkNavigator, retaining Needs you,
rich Wave/Task rows, filtering and scroll positions. Repo/Wave selection drills in;
Task/Session selection closes the overlay. Escape/outside click dismisses it.
The existing header control becomes wider and taller. The flat palette view and
its separate text-field implementation are removed; definition inspectors remain
in the existing repository menu. This supersedes Demo 4's popover design.

Review: the dropdown and docked sidebar share one view and navigation state.
Pinned Command-K focuses its existing search instead of mounting a second
navigator. Pane focus is withheld while navigation owns input, and the shortcut
monitor retains the original responder until dismissal. No terminal is released.
Check: `scripts/test_desktop.sh --filter 'WorkNavigationTests|WorkDestinationTests|SessionChromeProofTests|DesktopHeadlessTests' -Xswiftc -gnone --jobs 4` PASS (115 tests reported; three native tests skipped); platform boundary and diff checks PASS. `/tmp/loo487-dropdown-tests-verified.log`. The prior run exposed a sidebar-hide fixture that had not opened the now-default-closed sidebar; it now explicitly pins before testing Hide. Native dropdown geometry/focus remains demo judgment.

### Selection dismissal and split resizing

Jack Heart accepted Demo 5's layout, then canceled the shipping request to fix
selection dismissal: Wave and repository selections must close temporary navigation
just like Tasks and Sessions, including selecting the already-current destination.
Pinned navigation stays open. Shipping remains held for this review correction.
Jack also requested dragging dividers throughout the multiplexer, with sensible
size limits. Preserve nested panes, contents and focus; constrain both sides, show
a resize cursor, and adapt minimum sizes when the window cannot fit them all.

Jack Heart added golden-ratio snapping at 38.2% and 61.8%, and requested that new
shell splits start at 61.8/38.2 (existing pane/new shell). The initial Session/Task
browser split was initially 70/30, superseded by the Files default below. Resize minimums are 240pt
wide and 140pt high per pane, accumulated through nested splits and proportionally
reduced only when the viewport cannot fit them. A 10pt snap zone cannot override
minimums. Divider dragging uses screen coordinates so the moving divider does not
feed back into pointer movement.

Review: both split children now receive bounded frames; nested minimums replace
the old first-child-only limit. Screen-coordinate dragging avoids a moving-origin
jump. Accessibility increments use constrained free movement so repeated steps
cannot become stuck inside a snap zone. Shell creation changes only the inserted
split; retained pane identity and existing surrounding ratios survive.
Check: `scripts/test_desktop.sh --filter 'PaneResizeTests|WorkNavigationTests|MultiplexerStoreTests|DesktopPaneControlTests' -Xswiftc -gnone --jobs 4` PASS (104 tests reported, two optional native cases skipped); platform boundary and diff checks PASS. `/tmp/loo487-resize-tests-final.log`. Native pointer feel remains demo review.

### Initial Files and remembered layouts

Jack Heart requested trying the initial Session/Files split at 61.8/38.2,
superseding the earlier 70/30 Task-overview default. Existing window layouts
remain intact, including deliberately closed panes. A fresh demo window exercises
the default without deleting Session history or stopping existing providers.

Jack agreed that closed panes must stay closed and proposed database persistence.
LOO-416 already owns relaunch restoration; carry this direction there rather than
add a second persistence implementation here. Save the Work identity, split tree,
pane contents, ratios and focus. No saved layout gets the current default; a saved
single-pane layout is deliberate and must not recreate the browser. Restore
placement separately from process execution and state what survives quit/crash.
Current retention lasts only within the window; no database persistence is claimed.

Persistence handoff is saved here but not yet filed on LOO-416: its Task read
succeeded, but the owning Wave read failed with a Linear Project/Team mismatch.
No planning repair or duplicate Task was attempted.

Review: reuse the existing golden-ratio constant and Files page default. The
initial-layout guard still preserves resized and deliberately closed panes.
Check: `scripts/test_desktop.sh --filter 'DesktopPaneControlTests|TaskFilesTests|PaneResizeTests' -Xswiftc -gnone --jobs 4` PASS (48 tests reported, three optional native cases skipped); `git diff --check` PASS. `/tmp/loo487-files-default-tests.log`. Native review remains pending.

### File navigation revision — implementation authorized

Jack Heart requested the same default-closed dropdown/pin treatment inside Files.
The chooser shows only scratch and changed files relative to the Task branch's
recorded parent commit, including committed, staged, unstaged and untracked work.
Remove whole-repository browsing and the HEAD selector. Reuse the existing Task
changes snapshot: it already contains scratch paths and net changes at an exact
base commit. Filter both sections by path. Clicking a file closes the temporary
chooser; a pinned list stays open. Escape/outside click dismiss, with file drafts
and selected content retained. Cmd-P opens the chooser in the focused file pane;
Control-Command-B pins or closes it. The top control displays the selected path.
