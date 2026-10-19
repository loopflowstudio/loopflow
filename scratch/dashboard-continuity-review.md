# Dashboard continuity review — October 18, 2026

LOO-497 native review at `e08cbb102d236fb6046a39cb20b02b2f374e390e`.
Contract: [design.md](design.md). Ownership and remaining acceptance:
[desktop-api-coordination.md](desktop-api-coordination.md).

The agent exercised the actual Swift app, its matching CLI, and shared lfdb
writers/readers against disposable synthetic Work. This is partial native proof,
not configured-provider continuity or Jack Heart's acceptance. Jack requested
the demo; no new human feedback or acceptance arrived during this review.

## Observed experience

1. Product opened its normal conversation entry and Task inventory. The separate
   `review` Dashboard showed Paused (FIX-1), Idle (FIX-2), and Backlog (FIX-3).
   Backlog displayed its brief. The two started Tasks had no primary Sessions;
   their tiles explicitly said “No primary Session.” No provider was launched.
2. Hiding Paused collapsed its side to the remaining split. The native resize
   accessibility action changed that split from 50% to 52%. Shared plan revision
   4 still held three members, one hidden occurrence, and the full retained tree.
3. Restoring Paused restored its original 33% outer split and retained the 52%
   inner split. Clicking Idle selected one Task workspace; returning to the
   Dashboard preserved the composition.
4. Idle's Files tool opened `scratch/demo.md`. The first edit autosaved, as the
   default setting specifies, so it was **not** evidence of unsaved recovery.
   After disabling Autosave Task Files in the disposable app's Editing settings,
   a second edit displayed “Unsaved draft · retained in this window.” The shared
   pane record held that text, while the disk file still lacked the second edit.
5. A fresh process using the same private lfdb recovered the hidden occurrence,
   52% split, selected document tab, and second edit. Because this launch used a
   new app identity, its autosave preference returned to the default and saved
   the recovered text. This proves content recovery, not preference retention
   or an unsaved badge surviving a same-identity relaunch.
6. Product still showed its ordinary inventory after returning from the Dashboard.
   Restoring Paused after cold startup again preserved the 52% neighbor geometry.
   Final database counts remained zero Sessions and zero Task admissions.

Shared-reader evidence:
[initial plan](dashboard-continuity-evidence/plan-visible.json),
[hidden and resized](dashboard-continuity-evidence/plan-hidden-resized.json),
[restored](dashboard-continuity-evidence/plan-restored.json),
[unsaved pane intent before relaunch](dashboard-continuity-evidence/task-layout-before-relaunch.json),
[binary hashes and counts](dashboard-continuity-evidence/build.json).
Native observations came from the computer-use accessibility tree and screenshot
in Session `session_71f2e5969716428c84b8b8a9ff9b38bd`.

## Limits and preparation failures

- `desktop state` and native control target `com.loopflow.mac` in
  `rust/loopflow/src/lf/commands/desktop.rs`. The independent candidate has another
  bundle ID, so live candidate CLI/native round-trip parity was not demonstrated.
  The installed app was not substituted as candidate evidence.
- Another preparation appeared concurrently in this checkout and used the same
  initial demo bundle ID. Its note, `scratch/desktop-dashboard-demo.md`, was left
  untouched. Relaunch observations during that collision are excluded. The final
  candidate has unique identity `com.loopflow.mac.loo497review20261018` and is named
  **LOO-497 Review**. A fresh bundle path avoided stale app-identity lookup.
- An observation immediately after quitting relaunched the earlier bundle without
  the private environment and showed an installed-helper `layout` command error.
  That attempt is excluded. The final launch followed the development script's
  LaunchServices pattern, explicitly supplying private `LF_HOME` and matching
  `LF_BIN`. Production schema was not migrated.
- The minimal Rust selection fixture required a repository Git binding, complete
  Project metadata, and a Task base commit before native navigation/file browsing
  could read it. Those fixture errors were repaired only in disposable data.
- No live Session, shell Process, cross-machine transport, duplicate promotion,
  stale-write conflict, or anonymous-query arrangement was demonstrated here.
  The save-first anonymous-query choice remains unresolved in [questions.md](questions.md).

## Jack Heart's walkthrough — October 18, 2026 (afternoon)

Jack closed every running candidate because the duplicate **LOO-497 Demo** and
**LOO-497 Review** apps were indistinguishable. The agent relaunched only
LOO-497 Review on the private home (log: `/tmp/loo497-desktop-demo-20261018/jack-review.log`).

Jack's statements:

- “it sounds like were not using golden ratio defaults which i prefer (though it
  doesnt have to be like at all levels, it can be group A : group B is a golden ratio”
- On the `review` Dashboard screenshot: “looks kinda weird tbh”

Observed in the screenshot: three equal columns. Backlog's card fills its column;
the Paused and Idle cards shrink to the system empty-state view's height and float
mid-column with an oversized “No primary Session” headline.

Agent interpretation and action:

- Tile fill is a defect. `DashboardView.tile` never asked its stack to fill the
  split. Fixed in the working tree: tiles fill and top-align, and the empty state is
  a secondary label rather than `ContentUnavailableView`. Pending rebuild + Jack's look.
- Golden ratio is a design change, not yet agreed. Current default
  (`ArrangementTree::tiled`, `rust/loopflow/src/work/selection.rs`) halves members
  recursively, weights splits by member count (attention ×2), and uses one
  vertical axis everywhere. Proposal put to Jack: top split 0.618 : 0.382 with the
  first group (query order, attention-weighted) larger; equal below; optionally
  alternate axis per level. Open: is query order meaningful enough to justify a
  primary tile, and is alternating the axis wanted?
- After the fill fix was rebuilt and relaunched (agent screenshot: three full-height
  columns, top-aligned), Jack said: “i think the current layout is ok i guess”.
  Recorded as lukewarm acceptance of the fixed equal-column default. Golden-ratio
  defaults stay unadopted and unrejected — a noted preference for later, not
  current scope.
- Jack: “i dont know what the backlog is here though”. Cause: the disposable
  fixture titles Tasks after states (Paused/Idle/Backlog = FIX-1/2/3), and the tile
  header shows only the title — no Task identifier or state. Backlog (FIX-3) is an
  unstarted Task whose tile shows its brief. Agent proposal, not yet agreed: the tile
  header carries the identifier and state (e.g. `FIX-3 · Unstarted`) so a brief tile
  is distinguishable from a Session tile; future fixtures use realistic titles.
- Jack agreed (“yeah that works”) and set two requirements:
  - “make the headers stil lthe same regardless of single or dashboard view. the
    diference is whether you get the the sidebar views”
  - “need to be able to "zoom in" to a taks to get it to be the full view. Needs a
    different breadcrumb (goes through dashboard rather than prodcut)”
  Agent interpretation: “sidebar views” = the Task's Shell/Files tool panes, which
  only the full Task view offers. Implemented in the working tree, pending Jack's look:
  - `TaskHeading` (identifier · name · state, `TaskWorkView.swift`) heads both the
    window trail and each Dashboard Task tile; tiles also carry the same ⋯ Task menu.
  - `WorkNavigation.Destination.zoomed(dashboard:work:)`: the tile title or the
    expand button opens the full Task workspace (with Shell/Files); the trail reads
    `repo / review / FIX-2 Idle Started`, skipping the Wave; the Dashboard crumb
    returns. Back/forward record the zoom like any destination.
  - Open: should `lf desktop` express zoom-from-Dashboard (a CLI equivalent per
    the parity acceptance criterion), and should the Desktop inspection report it?

- After the sync relaunch, Jack on the Hidden strip (`Paused`, `Idle`): “why oes its
  show the state rather than the name?” Cause: the fixture Tasks were titled after their
  states, so the chips did show names. Jack then: “dont thikn we need to show state in
  the bottom, just the name”. Agreed: Hidden chips show the Task name only (`0d750ee778`).
  Fixture Tasks retitled in the disposable lfdb: FIX-1 “Retry flaky upload test” (paused),
  FIX-2 “Fix login redirect loop” (idle), FIX-3 “Add CSV export to reports” (unstarted).
- Jack asked to preview a Task with an actual Session. The candidate was relaunched
  with Jack's real `HOME`/PATH (provider auth) and the private `LF_HOME`, so a Session
  started from the zoomed Task view runs a real provider against disposable lf data.
  Pending Jack's look.

## Jack Heart's release and design notes — October 18, 2026 (later afternoon)

Installed release 0.14.7 (`3429242a3b`), Jack's loopflow window. Jack's statements:

1. “Updating... in top left” — sidebar stuck on Updating….
2. “no working tests found” — Working Tasks showed 0 (“No working Tasks in this list”).
3. “the chevron by the repo name is weird, would expect it to be a rop own launcher. ux
   needs to imply more that its breadcrumbs/search”
4. On zooming FIX-2 in the candidate (Brief/Changes, “Select a file”, no Session):
   “it should t be possible to get to a state where theres no session view, i.e. we
   shouldnt show task workspacdes for unstarted tasks”; “i am tempted to say you cant see
   unstarted tasks in queries, though its a bit tricky because its similar with completed
   tasks (may not have sessions left) but you probably DO want to be able to see sessions
   for recently completed tasks”; “could look at having sessions for unstarted tasks, or
   auto starting tasks, but i dont like either of those particularly”
5. “antoher thing i want to flag for review is the idea that queries have two purposes:
   what shows in teh sidebar and waht shows in the main pane” / “probably need to look
   more at the sidebar version first?”

Agent findings for 1–2 (diagnosis, not yet fixed): `lf desktop list` reports the window's
planning reading unavailable — “Workspace reader sent no observation for ten seconds”.
The installed app had no live `lf monitor work` child. Run directly, the first planning
frame (`lf monitor work --json`) was still streaming at 60 s: 8.2 MB, one line, 1,233
Tasks, unfinished, identically for 0.14.7 and this branch. It includes Waves whose
repositories are deleted test temp dirs (`/var/folders/.../T/.tmp…`), i.e. test fixtures
leaked into the production registry. So “Updating…” and the empty Working Tasks are one
failure: the machine-wide planning frame is too large/slow, the reader times out, and
the sidebar keeps the saved snapshot. Open: why the first frame is machine-wide rather
than repository-scoped, and where the leaked test Waves came from.

Jack: “Ok, this needs a performance test” / “and needs to be optimized”.
Done (local, not pushed):
- `planning_runs_git_per_repository_not_per_wave` (`tests/linear/work_watch.rs`): a `git`
  shim counts the commands one planning reading runs; adding 8 Waves on deleted
  repositories and 8 on an existing one must not add any. Failed before (7 → 38).
- `e0837d87b8`: missing paths fail without spawning git; the current checkout is found
  once per reading; the frame's Waves come from the roadmap instead of a second pass.
  On a VACUUM copy of Jack's lfdb (main + this fix, debug build, load avg ~45):
  git commands 340 → 137 per reading, identical 8.2 MB output; 6–15 s → 4–9 s.
- `fd8c98a873`: this branch's Dashboard plans carried a read-time `observed_at`, so every
  planning reading republished the whole frame after any Process write (pre-existing
  failure of `transcript_lines_do_not_publish_changes_or_delay_a_task`; now passes).
Remaining, measured: per-Task Flow membership (`flows_of_task`) scans all 4,246 Sessions
once per Task — 2.3 s for 622 Tasks; 3 git commands per live Task worktree (137 total);
every Task is serialized twice in the frame (roadmap.tasks and each Wave's tasks);
30 of 38 Wave repositories are leaked test temp dirs; Desktop gives the first frame 10 s.

Open design tension (4), not resolved: an unstarted Task opened as a workspace has no
Session view. Options Jack named and disliked: Sessions for unstarted Tasks; auto-start.
Jack's leaning: queries exclude unstarted Tasks — but completed Tasks share the
no-Session property and recently completed ones should stay visible.
Agent proposal: unstarted Tasks appear only as their brief and cannot be zoomed. Jack:
“i could accept just a start UI not unlike the "move to here" UI we already have”.
Direction (agreed in principle, not built): opening an unstarted Task shows its brief
with an explicit Start action styled like the existing “Open here”/move-here pane, never
an empty Files/Changes workspace. Starting stays explicit; no automatic start.

## Review disposition and next action

No design change was agreed with Jack Heart. Retain the current accepted contract;
this review supports the separation of membership, arrangement, and Task-owned
file intent, within the observations above.

Proposed implementation direction: make development/preview Desktop targeting
explicit and shared by opening, inspection, and controls, so `desktop state`
can inspect the same app receiving actions. Preserve passive readback. This is a
review proposal, not authorization to replace the installed app or add a second
state owner. Proof should compare candidate native inspection with the selected
Dashboard plan after hide/resize/restore, then exercise exact retained surfaces.

Next review: Jack can use **LOO-497 Review**, select Product → review, hide/restore
Paused, then open Idle → Files. Obtain Jack's judgment on the actual interaction.
Complete same-identity unsaved recovery and live Session/shell continuity with
the LOO-416 owner; current synthetic Tasks cannot establish that proof. Broader
acceptance stays with gate. No merge, Task completion, or Flow restart occurred.

Local candidate: `/tmp/loo497-desktop-demo-20261018/LOO-497 Review.app`.
Private data: `/tmp/loo497-desktop-demo-20261018/home`.
Fixture preparation and launch evidence: `/tmp/loo497-desktop-demo-20261018/`.
Use the recorded private environment when launching; opening the bundle alone
does not select the disposable Machine.

Check: `cargo build --bin lf` and `swift build --package-path swift -Xswiftc -gnone`
passed; native walkthrough and shared CLI reads above ran on those builds.

## Planning frame performance results — October 19, 2026

Measured on a VACUUM copy of Jack Heart's lfdb in a private `LF_HOME` (the draft
schema applied by hand), cwd `~/src/loopflow`, debug build, load ~11–15, median of 5.
621 Tasks, 52 Waves, 55 live checkouts.

| Stage | Commit | Frame | git | Time |
|---|---|---|---|---|
| Before (`5e08b6655f`) | | 8.27 MB | 135 | 4.70 s |
| Latest Flow per Task in one pass | `b2a7d855a0` | 8.27 MB | 135 | 1.35 s |
| HEAD and Git layout read from disk | `c62099c2d9` | 8.27 MB | 59 | 1.07 s |
| Roadmap writes each Task once | `f03d7cd21d` | 4.21 MB | 59 | 0.91 s |

Proofs: `every_task_latest_flow_matches_its_own_flows` and the Session-growth step
check in `process_membership_does_not_read_events_that_name_no_process`;
`planning_asks_git_once_per_live_checkout` (fails with the disk layout disabled:
3 commands per checkout); roadmap fixture round-trip in `dto_fixtures.rs`.
Desktop decodes `roadmap.tasks` once and gives each Wave its share; one Desktop
test that staged per-Wave Task unavailability alongside an available Wave was removed,
since that state cannot occur. Full Rust: lib 2038/2040 (`desktop_commands_do_not_route_to_the_execution_machine`,
`released_work_upgrades_to_saved_intent_without_rewriting_work` also fail on
`5e08b6655f`); other targets 696/701 (four `data_clones` need an un-nested sandbox,
one auth process count). Headless Swift: 600 passed.

Leaked Waves: all 30 temp-dir Waves are named `ship`, have no Tasks, and date from
July 20 and September 3. Source: `controller::wave` test
`resolve_registry_without_a_store_runs_unregistered` (added `22e34abc54`, deleted
`418e52ba65` on September 28) resolved the machine registry from the ambient home
and ensured a `ship` Wave for its temp dir whenever the dev schema matched. No
Wave was added by a full test run on October 18. Unpinned in-process tests can still
reach `~/.lf` when a branch has no draft migration; not changed here. Cleanup is
Jack Heart's call; nothing was deleted. They no longer cost git commands.

Machine-wide vs repository-scoped: keep machine-wide for now. The command palette
offers every repository's Waves and Tasks; scoping to the window's repository would
save only ~35% (406 of 621 Tasks are loopflow's). The remaining 4.2 MB is mostly Task
descriptions (1.47 MB) and PR evidence (0.75 MB), and 439 of 621 Tasks are completed.
Open: send descriptions on demand, or bound completed Tasks, before scoping.
