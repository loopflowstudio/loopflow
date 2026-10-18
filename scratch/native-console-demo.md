# Native Console demo — October 17, 2026

Jack Heart requested the Swift app demo. Accepted contract:
[navigation-implementation.md](navigation-implementation.md) and
[keyboard-shortcuts.md](keyboard-shortcuts.md). Complete earlier observations:
`9f4574310e03f73ae4d7b381ceac9da10e5251b5:scratch/native-console-demo.md`.

## Current evidence and next review

Operator owns packaging and the running demo; implementation does not replace it.
Recorded demo app (predates recovery repair): `/tmp/loo-489-native-demo/Console Demo.app`, bundle
`com.loopflow.mac.consolepreview`. Relaunch recipe:
`uv run --no-sync python /tmp/loo-489-native-demo/launch-demo.py`.
It uses synthetic Tasks in `/private/tmp/loo-489-native-demo/session-demo/real-home`
and installed real providers. Existing account configuration is unchanged.

- Jack Heart cut Open in editor. Operator's titlebar fixes built and a screenshot
  showed the complete DEMO-1 title, compact repo controls and no duplicate label.
- Real Codex answered; an unsent draft survived Cmd-K → DEMO-2 → Cmd-1 beside Files.
- Refreshed the signed preview from the implement build (68 focused tests passed),
  with its bundled CLI and 285 verified third-party files. Native resume now shows
  Codex correctly. The recorded review left DEMO-1 with Brief beside the real conversation.
- Native checks: Product shows all 3 started and 1 unstarted Tasks; opening DEMO-4
  does not add a working row. Cmd-1/2 switches Tasks. Cmd-K puts working Tasks first.
  Cmd-\\ hides/restores the sidebar. A Shell draft survived hide/show and Task
  switching; Brief stayed beside Codex. The test draft was cleared without running it.
- Real Codex → Claude reached first-run setup. Switching back before Claude had
  created a transcript failed with the missing-transcript error; this is not solely
  an offline fixture limitation. After fresh Claude produced a transcript, transfer
  back to Codex worked. Claude is not logged in in this demo path; no login was tried.
  Headless repair now checks history during requested-provider preparation, keeping
  the terminal and Session on refusal; a CLI regression proves retry after the
  transcript appears. Swift proves concurrent-update retention and observer restart;
  Rust/Swift prove new-turn clearing and lost/recovered preview behavior. This does
  not replace the recorded native failure or establish mounted UI acceptance.
- Fixture limits: planned Tasks report invalid planning evidence and Waves have no
  recorded objective. Missing sidebar IDs without issue URLs were a production
  display gap, now repaired in source; this demo predates that repair. These
  do not establish correct production planning/status behavior. Do not use this
  synthetic data for publication, completion, or automation tests.

Demo preparation is complete; this is not native product acceptance. Review in Light/Dark at narrow
and wide widths; Wave objective/status/actions and Started/Not started inventory;
passive unstarted Task preview; sidebar Cmd-F, Shift-F10 and focus restoration;
Shell/Files tabs, drafts, caret/scroll through toggles and Task switching;
filters/collapse/Back; two repository windows and Cmd-`; real provider transition,
new-turn status and observation recovery. Restart retention belongs to LOO-416.

## Screenshot comparison requested by Jack Heart

October 17: Jack Heart supplied native/prototype screenshots and requested an
intentionality review. These are findings/proposals, not new accepted direction.
Intentional: real provider terminal, optional Shell/Files arrangement, compact
pane identity when split, truthful summary fallback, optional Wave crumb, stable
working set and direct Waves. Screen sizes/zoom and open tools differ.
Drift: missing sidebar state edges; missing Waves label/working count; icon-only
toolbar plus unrelated delivery automation; repeated Task/path labels; inherited
rounded pane cards/gutters. Native metadata uses sans while the study uses mono.
The screenshot predates current source's unlinked Task IDs and Files/actions.
Demo descriptions are setup instructions, so identical Summary tails cannot judge
real activity quality. Shortcut hints intentionally appear only on hover/held Cmd.
Jack Heart accepted these recommendations except flattening: keep the existing
rounded cards, borders, shadows and gutters. His Rex reference requires no lower
content density. Restore labeled Shell/Files/actions, state edges and sidebar
hierarchy; move delivery automation into Settings; remove repeated pane labels
and consolidate tool controls. Preserve actual terminal content and capabilities.
After seeing the labeled toolbar, Jack first selected the existing Codex pane
header, then superseded that with “drop all 3 for now.” Shell/Files/actions buttons
are removed from visible headers. Existing View commands and shortcuts remain;
Cmd-/ and sidebar context menus retain Task actions. No extra header row is added.

## Build boundary

Screenshot follow-through: no visible Shell/Files/actions header buttons; delivery controls in
Settings; counted sidebar groups, state edges and mono metadata; quieter row
selection; Task pane paths on hover (visible for differing locations); Shell tabs
in the 28-point pane header; Files controls in its 30-point tab row. Rounded pane
geometry/shadows/gutters are unchanged. Review caught a Brief → file-browser
transition that must retain a pinned tree; the focused interaction test covers it.
Verification: `scripts/test_desktop.sh --filter 'TaskRowMetadataTests|TaskFilesTests|WorkspacePanesTests|ConsoleWorkspaceTests' -Xswiftc -gnone` — build and 40 tests pass; `git diff --check` passes. Latest header removal: `scripts/test_desktop.sh --filter 'ConsoleWorkspaceTests|WorkspacePanesTests' -Xswiftc -gnone` — build and 7 tests pass. Signed preview refreshed. Native checks confirm no Shell/Files/actions header buttons, working Ctrl-` and Shift-Cmd-E toggles, and Cmd-/ Task actions. Shell tabs occupy the existing pane header; Files controls remain in one tab row. DEMO-1 was reopened beside Brief; its prior provider client remains attached elsewhere, so the preview offers Move here without forcing takeover.

`scripts/test_desktop.sh` builds the app and runs production model/leaf tests
with WindowServer denied; it cannot establish mounted terminal behavior.
Build/package with `uv run python scripts/loopflow-dev.py build` and
`cargo build -p loopflow --bin lf`. The candidate has `session_preview.sql`;
use `docs/data-clones.md` for a supported private live-data clone. Copied Sessions
have continuation limits; disposable native input proves retention separately.
The installed store and installed app were untouched. Publication belongs to the
Flow's delivery step; merge and Task completion remain excluded.

October 18 gate repair: toggling both tools on before opening a conversation, then hiding both, now retains **Open conversation** and offers **Show pane**. The production leaf-view regression proves entry remains reachable and revealing a pane preserves its layout. Mounted input remains a separate demo boundary.

October 18: Jack Heart reported a small sidebar jump on hover. The slot hint reserved width but changed Text height when the Command glyph appeared. Metadata now measures a hidden shortcut continuously and overlays its hint/age, keeping hover out of layout. `scripts/test_desktop.sh --filter TaskRowMetadataTests -Xswiftc -gnone` — build and 11 tests pass; `git diff --check` passes. Signed demo rebuilt and reopened. Native screenshots show stable row positions across selection; the background automation did not expose a shortcut hint, so actual pointer-hover review remains with Jack Heart. Fresh Task status remains active, no PR; previous pursuit is completed and its stale predecessor has no live process in lf ps.

October 18 delivery direction: Jack Heart explicitly requested `lf ship -b` and landing, followed by a patch release. This supersedes the earlier wait-for-native-review publication hold. Gate should repair concrete readiness defects and retain honest evidence limits; do not require another design approval or replay pursuit. Preserve rounded pane framing and the latest removal of all three header buttons.
