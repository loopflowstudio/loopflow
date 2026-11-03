# Wave lifecycle demo — 2026-11-02

Jack Heart requested a PR walkthrough, then a hands-on demo in Loopflow Dev, and
asked to run `lf sync` first. No acceptance or merge instruction was given.

## Revision and setup

`lf sync` merged main `ece53eec1b0274bd549525be5efbdc0bfb3965f2` without
conflicts. Published PR #1803 and the local branch now name
`0d7d1cb378d37d18331c2ff7e7e6d93330be7d12`. The first demo build was stopped
before sync; the second build uses that merged tree.

A fresh `lf-world build wave-lifecycle /tmp/loo-500-synced-demo` completed.
It contains research (empty), desktop (three open Tasks, one started, one completed,
a Wave conversation), and archive (completed work). Provider history and work
are fixtures, not a running real Graph or a live Linear account.

Launch command: `uv run python scripts/loopflow-dev.py run-debug --world
/tmp/loo-500-synced-demo`. Its private LF_HOME is
`/private/tmp/loo-500-synced-demo/home`; real Task and Wave data is not selected.
Build/launch logs: `/tmp/loo-500-synced-demo-launch.log` and
`/tmp/loo-500-relinked-demo-launch.log`.

## Observed evidence

The isolated CLI demo at pre-sync revision `5f96325478` added research, renamed
it discovery, previewed removal, and removed it with main detached. Removal
returned exit 0 with an explicit uncommitted-file receipt. After checking out
main, repeating the removal committed its saved deletions; Git status was clean
and HEAD was `867ec29 lf wave remove: discovery`. The live list was empty while
`lf wave show discovery` retained its chapter. Rename also reported bare-word
prose mentions, as designed. Exact output: [CLI transcript](wave-lifecycle-cli-demo.txt).
The sync changed none of the lifecycle source used by this trial.

Native launch failed before a usable review. The first app executable had not
relinked against the newer Rust archive; removing only the generated executable
and rebuilding forced the link. The fresh private-world launch still exited.
A direct launch with the world's environment reproduced exit -4. LLDB stopped
on `EXC_BAD_ACCESS` in `<loopflow::lf::Commands as
clap_builder::derive::Subcommand>::augment_subcommands` on a Swift cooperative
worker thread. This is consistent with stack exhaustion, but the exact cause
has not been established. Evidence: [debugger output](wave-lifecycle-launch-failure.txt).

Attempted UI inspection after those exits opened the Dev app without the private
environment; it displayed the shared-store unmerged-migration guard. That error
is separate from the private-world crash. No shared-store migration was run.
There is no successful native before/after capture and no acceptance from Jack
Heart. The app has been closed; the native runbook cannot yet be completed. Successful live Linear recovery and process-kill
recovery at every boundary remain unproved. Publication notes also retain the
pre-existing install-scheduler fixture failure; the gate is not wholly green.

## Review route

Use [demo.html](demo.html) to add ops, enter its workspace, return to the repository,
rename ops, remove research, then preview and remove desktop. Inspect where its
three open Tasks go and whether the started Task retains its checkout and Session.
The fixture proves retained records, not actual live-process survival.

The current [design](jack-loo-500-make-adding-and-deleting-waves-seamless.md) keeps
recursive retirement, parent/repository rehoming, CLI-only `--delete-backlog`,
Initiative archival, and authored-file changes on main. No new design change was
agreed during setup. Recommended next action: repair the native startup crash, relaunch this private
world, then have Jack Heart try navigation, consequence text and recovery.
Keep the launch failure as review evidence; no additional Graph was started.

[Implementation walkthrough](pr-review.html) pins source excerpts to the synced
base and head. Desktop and narrow browser layouts were inspected; code scrolls
within its own pane without overflowing the page. No product code changed for
these artifacts.

## Relaunch requested — 2026-11-03

Jack Heart requested another `lf sync` and Dev demo launch. Sync merged
`2bc29a72e0` without conflicts, producing `f124694bab`. The earlier startup
crash matches LOO-673's diagnosed Swift-worker stack exhaustion. Its existing
two-file repair from `e6a8273bc6` was applied here: in-process library calls
run on an 8 MB thread, with the existing regression test. This is a prerequisite
for the demo, not a Wave lifecycle design change. The focused regression passed:
`scripts/test_desktop.sh --filter LoopflowLibraryTests -Xswiftc -gnone < /dev/null`
(1/1). The repaired executable was installed and signed, and the same private
world launched successfully. Accessibility inspection confirmed the repository
window, Archive/Desktop/Research Waves, and Task inventory. Native screenshot
capture was unavailable. No Wave was added, renamed or removed in this pass;
Jack Heart's hands-on acceptance remains pending. The earlier crash is retained
as historical evidence, superseded for startup by this successful launch.

## Sidebar review — 2026-11-03

Jack Heart observed that New Wave was visible after selecting a Wave but not
when the repository was selected. The action was the last child of the bounded
Wave scroller and also hidden until hover. Moving a selected Wave to the header
freed one row; repository selection put it back and pushed New Wave out of view.
The repair pins New Wave below the scroller and makes it continuously visible.
Native accessibility inspection after rebuilding confirmed New Wave outside
the scroll area with the repository selected. The app is open for comparison.

Jack Heart also asked what “Still mentioned in” does. Source inspection shows a
case-sensitive word-boundary search for the old slug's leaf name in other Waves'
GOAL.md and MEMORY.md. It reports file:line locations without changing prose or
blocking the lifecycle operation. Explicit authored path rewriting is separate.
The wording “Old name still appears in these documents; review if needed” was
suggested in conversation; no copy change was requested or applied.

Jack Heart then rejected the prose-mention feature entirely (“I just dont want
this to exist at all”). Remove its scan and warning, rather than changing its
wording. Keep explicit path updates and leave ordinary prose untouched. This
supersedes the suggested copy above. Verification passed: lifecycle integration 14/14; Wave DTO round trips 5/5;
`cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`;
`scripts/test_desktop.sh --filter 'DTOFixtureTests|RenameWaveSheetTests|RemoveWaveSheetTests' -Xswiftc -gnone < /dev/null` (57/57).
New Wave's repository-scope button also opened the native creation form; it
was dismissed without creating data. The warning-free app and bundled CLI were installed and signed, then reopened
on the same private world. Accessibility inspection confirmed the repository
window and the fixed New Wave footer. Changes are published as `57720b602e`.
Native hands-on acceptance of the footer position remains open.

For the requested comparison, current official docs show Linear team creation
in Settings → Your teams → +; Figma folder creation on the team/folder page via
+ Folder or + Create; Slack channel creation via its separate sidebar + action.
These are documented entry points, not live verification of their scrolling.
Sources: https://linear.app/docs/teams ;
https://help.figma.com/hc/en-us/articles/360038006494-Create-and-manage-folders ;
https://slack.com/help/articles/201402297-Create-a-channel .
The proposed Loopflow footer keeps New Wave visible independently of list length
and selected scope, at the cost of one permanent row. No final placement choice
from Jack Heart has been recorded.
