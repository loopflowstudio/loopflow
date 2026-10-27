# Workspace frame — LOO-608

## Current direction — 2026-10-27

Jack Heart reviewed the extra workspace edge and said, “i just dont think we
need the extra border”. Remove the outer workspace decoration entirely.
Keep both top corners square and both lower corners rounded. Individual pane
focus styling, layout, terminal sizing and input behavior remain unchanged.

The workspace now uses only `PaneGeometry.workspaceShape` for clipping; its
border/shadow overlay is removed. The geometry test retains its Light/Dark ×
split-axis coverage of corner shapes and lower-corner clearance. Assertions
about the removed shadow overlay are deleted. The README matches this direction.

## Superseded implementation

The October 26 brief requested a fading edge. The first implementation used a
two-point shadow-colored stroke at 30% opacity, blurred by three points. Its
Desktop build and four focused geometry cases passed before publication as
PR #1728. The October 27 concept review accepted that implementation for visual
review; Jack Heart's later feedback supersedes its shadow and tuning guidance.

## Remaining work

Jack Heart reviewed the relaunched border-free build on October 27 and said,
“looks fine, ship it”. Visual acceptance and shipping are approved. Run the
remaining delivery checks, land PR #1728, and confirm Task completion.

Check: `scripts/test_desktop.sh --filter TerminalPaneLayoutTests -Xswiftc -gnone` — PASS on the border-free revision: Desktop build and four Light/Dark × split-axis cases.

Review: the workspace now clips without drawing an outer edge; pane outlines
and lower-corner clearance remain unchanged. No new state or input handling.

## Relaunch failure — 2026-10-27

Jack Heart supplied a startup screenshot reporting that `lf config repository`
was unrecognized. The initial relaunch copied only the tested Swift executable
into Loopflow Dev, leaving an incompatible bundled CLI. A running app process
and matching Swift UUID did not establish a usable workspace.

The bundled helper's help listed only `config user`; this checkout defines
`config repository`. Recovery uses the full `scripts/loopflow-dev.py run`
installation, including its matching helper and repository-opening preflight.
The default-home development copy notice is expected and unrelated to command
parsing. No copied database was deleted.

Recovery completed through the full development launcher: Swift and its matching
CLI built, bundled resources verified, signing succeeded, and the repository
opening preflight passed. Native UI inspection then showed the Updating indicator
clear and eight Tasks plus Wave rows load without the startup error. The retained
repository Session offered Resume; no conversation was resumed for this check.
This proves startup recovery, not native terminal continuation or Jack Heart's
visual acceptance of the frame.

## Demo acceptance — 2026-10-27

Jack Heart accepted the relaunched appearance and authorized shipping in the
primary Task conversation: “looks fine, ship it”. This supersedes earlier
notes that visual acceptance and merge approval were pending. No separate
Light/Dark or terminal input matrix was claimed. The startup mismatch was
repaired with the full matched app/helper installation before this acceptance.

## Headless demo decision — 2026-10-27

Advance. This review stands in for Jack Heart using his recorded acceptance,
not a newly inferred visual judgment. The reviewed experience is the border-free
Desktop workspace with square top corners and rounded lower corners. The current
design is [Current direction](#current-direction--2026-10-27); no further design
changes are proposed or agreed in this headless review.

The current source at `420e7ebd0b12eace0611c76a360f7121f6fd6378` matches that
direction: [workspacePanel](../swift/LoopflowMac/Views/SessionsView.swift) clips
without an outer overlay, and [PaneGeometry](../swift/LoopflowMac/Views/WorkspacePanesView.swift)
squares both top corners. Only this review note had uncommitted changes at
inspection. The recorded Desktop build and four
[geometry cases](../swift/LoopflowTests/TerminalPaneLayoutTests.swift) remain
applicable; they were not rerun solely for this stage.

Check: `uv run python` invoking `_check_repository_opening(Path.cwd())` from `scripts/loopflow-dev.py` — PASS through `/Users/jack/Applications/Loopflow Dev.app/Contents/MacOS/lf`, using the launcher's configured environment and 30-second timeout.

This fresh configured-path check confirms repository opening through the bundled
helper. The earlier native observation and Jack Heart's “looks fine, ship it”
remain the visual evidence; this headless run captured no screenshots and makes
no new rendering claim. The Task status read also records that acceptance in its
shipping event, with Workflow position `demo` and
[PR #1728](https://github.com/loopflowstudio/loopflow/pull/1728) still unmerged.

No unresolved product decision or concrete implementation defect remains for
this frame change. A separate native terminal input/resize and Light/Dark visual
matrix remains unperformed, as previously recorded; geometry coverage is not
claimed as that proof. The [concept page](concept-review.html) is an earlier
schematic, and its pending visual-review wording is superseded by the acceptance
above. No interactive demo page is generated on this headless surface.

Next action: continue delivery checks and landing, then verify the merged change
and complete the Task. This decision advances demo only; it does not claim a
merge, complete the Task, or restart a Graph.

## Gate — 2026-10-27

Reviewed PR #1728 against its `main` base, including the existing uncommitted
acceptance notes. No production-code repair was needed. Added an assertion to
the four geometry cases that the workspace has no outer overlay; updated the
concept page's obsolete pending-acceptance text. Pane strokes, terminal sizing,
focus and input paths are unchanged. The change removes a drawing operation and
adds no state, I/O or layout pass; no performance improvement was measured.

Check: `uv run python scripts/test.py --loopflow --reuse-passing` — RESOURCE BLOCKED before any suite: 11.5 GiB free versus the 32 GiB reserve after automatic recovery; Swift app/tests and Xcode fallback build deferred to capable CI.
Check: `uv run python scripts/check_architecture.py` — PASS, all seven architecture coverage categories.
Check: `uv run python scripts/check_swift_multiplatform_boundaries.py` — PASS.
Check: `git diff --check` — PASS.

The resource scan, not a product test, consumed the initial wait. Its optional
uv-cache cleanup failed immediately on an in-use cache lock; active/recent and
unresolved build roots were retained. No test timed out or ran in this gate.
The runner's receipt records `resource_blocked`; diagnostic output is retained
at `.lf/tmp/gate/run-67738/resource-preflight.log`. The prior geometry pass
predates the new no-overlay assertion, so that assertion remains unverified
until CI runs. Delivery must clear added scratch artifacts to enable candidate
CI, as documented in TESTING.md; skipped checkpoint jobs are not passing checks.

Remaining documentation handoff: the branch's `.lf/wave/product/MEMORY.md`
paragraph still describes the superseded blurred shadow and pending visual
acceptance. Gate leaves Wave memory read-only. At the next authorized memory
maintenance step, replace that account with: “October 27, LOO-608: Jack Heart
removed the extra outer border entirely and accepted the relaunched border-free
workspace with square top corners and rounded lower corners. Geometry coverage
does not establish native terminal input/resize or a full Light/Dark visual
matrix.” Preserve the earlier nested-clearance history.

Gate can advance with explicit CI ownership of unavailable build/test checks;
this is not a full verification pass or permission to bypass required CI.
