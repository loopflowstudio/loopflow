# Current LOO-489 Console contract

The full accepted contract, native demo and keyboard map are preserved in git at
`8d4f2d8ea1430fd41d946d328145ce87e26c7b7b:scratch/history/loo489-console-source-2026-10-17/navigation-implementation.txt`,
`native-console-demo.txt` and `keyboard-shortcuts.txt` in that same directory.
These paths share that commit and directory. The manifest retains source/base paths
and verified hashes. Integration procedure:
[native-console-integration.md](native-console-integration.md).

- One titlebar: repository conversation crumb + separate navigation chevron,
  optional Wave, Task identity, labeled Shell/Files and contextual actions.
- Stable working Task sidebar; activity never reorders. Explicit pause removes
  working rows/digit slots but retains stored order/workspace and search/Wave access.
  Filters preserve the center and never silently clear on number navigation.
- Repository opens its conversation. Wave opens its conversation beside all
  current planned/started Tasks, with objective/status and existing plan actions.
  Unstarted Task opens only its brief; navigation neither starts nor acquires work.
- Task has its existing Session, one Shell tool with tabs, one Files tool with
  tabs including Brief/Changes. Hide/show retains clients, selection and drafts.
  Repository/Wave have no Shell/Files. Custom Dashboard/grid/API stays LOO-497.
- Cmd K Find Work; cross-repository selection uses another window. Cmd-number is
  stable working order, Ctrl-Tab adjacent Task, Cmd-brackets history.
- Preserve actual native terminals and the existing ownership/navigation owners.
  Latest Session text needs current turn/attachment/source evidence; otherwise
  labeled summary. No fake live state or another preview acquisition engine.
- Latest native review accepted labeled tools, sidebar state edges/hierarchy and
  removing duplicate labels; retain rounded cards, borders, shadows and gutters.
  Jack Heart explicitly cut Open in editor. Do not revive it as a blocker.
- 489's provider-switch repair preserves the retained terminal on missing history;
  late metadata updates remain valid and retry is possible. Preserve this repair.

Headless checks and partial native observations are documented in the snapshot.
Neither proves the complete mounted-retention/restart matrix; lifecycle remains
LOO-416. The captured source is integration input, not a combined passing build.
