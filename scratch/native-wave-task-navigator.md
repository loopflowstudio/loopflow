# Native Wave / Task navigator

Jack Heart approved native implementation on October 15, 2026: prototype 03,
program headers (4), collapsible sidebar (5), and a compact attention queue.
Switching contexts must preserve the last destination, native clients, drafts
and list positions. Shader/contrast and app theming are separate work.

Replace the chevron outline with stable Wave and Task columns. Repository is a
selectable destination; the Wave column restores its remembered destination,
while a separate Wave conversation target selects the Wave itself. Existing
WorkModel selection, retained workspace registry and Rust readings remain the
owners. Waiting/Blocked evidence adds a bounded queue above the columns; queue
navigation uses the same selection path and Back history. Search hides unrelated
Waves and filters Tasks without changing selection or organizational order.
Remembered Wave scope determines the displayed column, never selection styling.
Collapse changes only width.

Delete — do not maintain: WorkNavigator's folding/indented row presentation.
Keep shared projection eligibility, metadata links, terminal Task retention,
error disclosures and retained workspace identity. Program headers expose
existing provider/title/path/status, with a Shell fallback and no detector.

Implemented in the existing navigation/workspace owners: 430px two-column
sidebar, selectable repository, per-Wave destination and Task-list scroll,
explicit retained Wave conversation through `lf session ensure --wave`, bounded
three-row attention queue with overflow, and sidebar toggle / Control-Command-S.
Nonempty panes show program headers. Removed obsolete navigation folding state.

Review fixes: attention delegates the whole opening to the existing Session
owner so a Session retained under another Work adds only one Back entry;
shared Session memberships produce one attention item, preferring recorded
checkout ownership; runtime/planning Task IDs share the selected row. Single-Wave
visibility, exact attention opening, Back, retained view/pane/draft identity,
context return, truthful status and program names have headless coverage.

Check: `scripts/test_desktop.sh --filter WorkNavigationTests -Xswiftc -gnone` — app/test build and 43 tests PASS after search/selection review fixes (`/tmp/loo462-sidebar-search-tests.log`); earlier metadata/polish checks remain applicable.

Remaining acceptance: native visual/keyboard-focus/scroll demonstration and
fallback Xcode gate. Headless retained-view identity does not prove mounted PTY
or rendered focus behavior. The [native demo](sidebar-demo.md) was subsequently
launched at Jack's request. Search/selection fixes are committed after that build.
The [two-level workspace design](task-workspaces.md) owns subsequent exploration
of breadcrumbs, nested workspaces, sidebar resizing and keyboard-only history.
Earlier broader check found two existing WorkCacheTests failures
(`returningLaunchRestores`, `refreshAfterSelectionChange`): they expect a saved Task
selection at launch, while committed `WorkModel.restore` deliberately restores
readings and starts at the repository. That existing contract is unchanged;
`/tmp/loo462-navigator-tests-2.log` retains the evidence.
