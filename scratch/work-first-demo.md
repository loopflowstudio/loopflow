# Work-first native demo

Jack Heart requested real configured-repository review, main Work views before
Portfolio. Initial installed-CLI mismatch, failed launch and stacking history:
`ce367d7143:scratch/work-first-demo.md`, opening sections. Those failures remain
unaccepted; no live installation or landing is authorized. Current design:
[get-me-using-lf.md](get-me-using-lf.md), PR #1531.

## Isolated branch demo — October 11

Jack Heart requested a compiled branch CLI against a clone, then asked to launch
it. Built the current branch and packaged it as `/private/tmp/loo462-native.FgtDmE/Loopflow Portfolio Demo.app`
with separate bundle ID `com.loopflow.mac.demo.loo462`; existing Dev and production
apps were left running. Fresh `LF_HOME`: `/private/tmp/loo462-native.FgtDmE/home`. Disposable clone:
`/private/tmp/loo462-native.FgtDmE/repos/loopflow-demo`. The clone has no origin remote and its local config
has no Linear binding; demo-only instructions prohibit operations on live Work.
No existing database, provider history or credentials were copied.

The private portfolio root is `/private/tmp/loo462-native.FgtDmE/repos`, not the real ~/src. The packaged
CLI's actual `portfolio --json` and `repo identity --json` calls succeed under this
Home and report only the clone. App launch succeeded and its process was observed;
the Portfolio URL was sent explicitly to this app. Logs: `/private/tmp/loo462-native.FgtDmE/app.log` and
`/tmp/loo462-isolated-package.log`. App log contains AttributeGraph cycle warnings;
no rendered usability or user acceptance is inferred from launch success.

Next review: Portfolio Folder/Manual controls, selecting the clone, then repository
conversation entry. New provider conversations may use normal provider login; none
of the live Loopflow history is present. This demonstrates the isolated composed
branch, not installed real-data migration or existing conversation continuity.

## Review priority — October 11

Jack Heart reported that the isolated app did not load and would not quit, then
force-quit it. The empty clone did not demonstrate the intended populated
Loopflow repository experience. Jack redirected review to the main repo, Wave
and Task views: verify the expected Waves and Tasks appear, and inspect whether
new buttons or chrome have been added. Portfolio remains wanted but needs further
revision; review it second, after the main views feel right. This is not native
acceptance of either surface.

The reproduced freeze sampled a main-thread rendering loop. SessionsContentView
construction and rendering used different repository path spellings and repeatedly
changed the retained workspace location. Construction now uses the Session store's
resolved path. The regression failed before the fix; focused WorkNavigation and
WorktreeWorkspace suites passed afterward (44 tests). The repaired isolated app's
sample shows an idle event loop, not the previous redraw loop. Rendered usability
and normal Quit remain unconfirmed; screenshot capture failed and native scripting
provided no usable inspection. Logs and samples remain under
`/private/tmp/loo462-native.FgtDmE/`; test logs `/tmp/loo462-location-{before,after}.log`.

Next demo must center the populated Loopflow repo and compare its displayed
Wave/Task inventory with shared planning, not substitute an empty portfolio demo.
Keep live history and execution intact; no cleanup or client takeover is authorized.

Populated-demo preparation: copied the live SQLite database using its read-only
backup API to `/private/tmp/loo462-native.FgtDmE/populated-home/loopflow.db`.
The branch's packaged CLI refuses the copy: development schema does not match this
experiment. `repo identity --json` fails; `wave list --json` misleadingly returns
an empty list alongside the same incompatibility warning. That empty list is not
an inventory. No populated app launch or live migration followed. The earlier
installed-app launch opened obsolete Concerto; Jack Heart rejected it as the wrong
build. The requested demo remains this diff with real populated planning, not that
release or the empty clone. Remaining blocker is preparing a compatible isolated
planning dataset without changing the live store or losing the relevant inventory.

Populated branch demo launched (October 11): the disposable copy was prepared
using the existing `apply_project_content_cut` test helper and validated with
`validate_experimental_sqlite`; the temporary ignored test was removed afterward.
Preparation passed, including project-content conversion and journal seeding
(`/tmp/loo462-populated-prepare.log`). Exact before/after IDs match: 55 Waves,
52 Projects, 546 Tasks across the snapshot. Packaged repo identity and Wave reads
now succeed. Loopflow's snapshot contains ready growth/infrastructure/intelligence/
product/spend and abandoned engbot/list; raw inventory is not the displayed filter.

Launched the repaired branch bundle directly under `populated-demo.sb`, using
`populated-home`, against `/Users/jack/src/loopflow`. Sandbox blocks network,
live-file writes, provider execution and signals to outside processes; copied
planning is for layout/navigation review, not running agents. PID 33874 and a
1280×800 on-screen window were observed. This is not the obsolete installed
Concerto app. Native rendering and correct displayed Task membership still need
Jack Heart's review; no screenshot or conversation-continuity proof is claimed.
Main-view source comparison removes the generic New Session controls and adds
the previously requested collapsed Overview disclosure; it is not zero new chrome.

Direct inspection correction: Jack Heart's screenshot showed repository validation
failure, not a populated view. The demo sandbox allowed `/usr/bin/git` but denied
its Xcode executable and the resolved Homebrew executable. Reproduced the exact
sandbox error, allowed those concrete Git executables, and confirmed rev-parse.
Native Apple-event inspection and screenshots now work; screenshot
`/tmp/loopflow-20261011-200721.png` visibly shows “Loading work…” and a spinner,
not Waves or Tasks. The app is responsive; its work-watch child is CPU-bound in
SQLite while constructing Task detail snapshots (sample `reader.sample.txt` in
the demo directory). The capture-on-launch helper instead selected the transient
launch window and failed after it closed. Neither window existence nor an opening
receipt proves the requested rendered view. No native acceptance is established.

Loading repair: repeated samples located the delay in Flow membership reads. The
snapshot holds 299,139 Processes and only 250 Flow records. Flow reads now restrict
candidate process IDs before applying the unchanged shared checkout, Session-history
and attachment membership rules. No schema/index or stored-history change. A direct
snapshot SQL probe for LOO-462 returned the same five rows (0.19 s before, 0.057 s
after using Python SQLite; not an app latency benchmark). Five Rust task-work tests
pass, including candidate filtering preserving historical association outside the
checkout. The branch CLI was rebuilt and repackaged in the isolated app.

Verified native result: `/tmp/loopflow-20261011-201523.png` shows a populated
Growth/Infrastructure Task sidebar and “Choose Work” in the repository tab, not the
loading spinner. Native inspection reports `reading: current`; its opening receipt
still incorrectly reports cancellation. Visible conversation counts/dropdowns remain,
and repo-tab conversation-first behavior is not demonstrated. These are review
findings, not acceptance. Other Waves below the viewport and full exact Task membership
have not yet been visually verified. The macOS “cmux was prevented from modifying…”
notification is retained as an unexplained observation, not proof of a successful write.

Check: `cargo test -p loopflow --lib store::sqlite::task_work::tests` — 5 passed;
`cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` — pass.
Native capture proves first populated rendering, not complete inventory, input,
conversation continuation or final review acceptance.

October 11 sidebar revision (`63a20694d5`): removed numeric Session menus and fixed
terminal Tasks remaining current solely through retained conversations. Rebuilt and
launched the private app; direct capture `/tmp/loopflow-20261011-213145.png` still
shows loading, not the revised populated inventory. A fresh shared-reader probe
reports a YAML parse error in the separately changing main checkout's config.
The data audit and 36 navigation tests establish the local correction, not a
successful refreshed native demo. No source or destination database cleanup yet.

Subsequent direct capture `/tmp/loopflow-20261011-213231.png` and native inspection
show the revised populated sidebar with no numeric conversation controls and a
current reading. The transient main config error cleared without edits here. This
supersedes the failed refreshed-view observation above, not the recorded failure.
The latest stopped-failure projection correction has focused Rust and Clippy passes;
it has not yet been packaged into that captured app.

## October 11 native navigation and cleanup rehearsal

Repackaged `c229a29648` into the same private app and launched PID 53987. Native
Computer Use inspected the actual populated window and clicked Product, then
LOO-462. The sidebar has no conversation tallies; completed LOO-440 is absent.
Product opens in a retained Work tab and LOO-462 opens its conversation pane with
collapsed Overview. The provider launcher is refused by the intentional account-free
sandbox; this is not successful conversation continuation. Wave workflow discovery
also encounters the sandbox's denied user skill directories. The demo policy needs
read-only skill access, not provider credentials or a product-code workaround.
Product capture: `/tmp/loopflow-20261011-214224.png`.

The Product plan displays old Tasks, including LOO-149. Read-only comparison proves
LOO-149 has the same `abandoned_at=1790172305`, null planning state and old Project
in live and copied databases. The copy did not create that mismatch. The shared
reader deliberately includes other Projects' backlog; do not hide every historical
Project or delete its Tasks to repair a terminal/planning mismatch. Determine whether
normal present-day abandonment still creates this combination before choosing code
versus one-time data repair.

LOO-376's current brief explicitly records Jack Heart's earlier closure request
and retirement of numeric acceptance. Rehearsal backup:
`/private/tmp/loo462-native.FgtDmE/before-376-cleanup.db` (SQLite backup, mode 0600).
The first sandboxed `task complete` saved a request then failed obtaining the real
checkout's PR mutation lock. Retry policy permits only that exact advisory-lock
file; all real repository/data writes and networking remain denied. The retry has
committed completion in the private database; live LOO-376 remains unstarted.
Native sidebar refresh removed LOO-376 without restart, preserving adjacent paused
LOO-378 and unfinished LOO-371. Cleanup subprocess 81008 remains live while checking
retained execution, so its final cleanup result is not yet established. Logs:
`376-cleanup-{rehearsal,retry}.log` in the demo directory. Do not apply the private
full-database snapshot over the live database; ongoing unrelated writes must survive.

Rehearsal finished with exit 0: LOO-376 completed locally and its checkout was
retained because execution observations were unresolved. No history was deleted;
this does not establish live process state from a snapshot. The earlier live-process
note above is superseded by this terminal receipt. The real database is unchanged.

### Installed cleanup and Workflow rendering

The private app now permits read-only access to user skill-definition directories,
not provider credentials/history or provider execution. Native Product view renders
the feature Workflow (start, design, demo, end); the earlier sandbox error is gone.
No app code or installed configuration changed for this correction.

Live backup: `live-before-376-cleanup.db` in the demo directory, SQLite-consistent
and mode 0600. Initial live completion saved a request but refused unresolved old
follow-through scope. No completion was claimed from that failed command. The
installed binary was then rehearsed against `installed-rehearsal-home` with that
backup's original schema: record no further performance follow-up under Jack Heart's
October 7 retirement/closure decision, then complete. Both returned exit 0; the
checkout was retained for unresolved execution. Early sandbox dispatch and obsolete
`--repo` spelling attempts failed before the successful rehearsal and are logged.

Applied the same supported follow-through/completion operations to live LOO-376.
Fresh read shows `planning_state=completed`, `planning_completed=1`, no pending
completion request/error. Its final cleanup process is still being observed; the
snapshot's unresolved process IDs never established live liveness. Other Tasks and
historical Sessions were not retired. No snapshot was restored over the live store.

### Verified live result and legacy cleanup

Live LOO-376 completion returned exit 0; fresh supported status says done. Its
checkout remains because ef54b06d-9896-467f-a920-f8d4648f9d8b and
e2d3d8d6-a9d5-43ef-83d8-022aadd47295 have unresolved execution evidence. No attempt
was made to erase those records. This supersedes the pending-process note above.

The one-time missing-disposition repair selected 126 exact already-abandoned rows;
all had no subsequent Task edit/comment, no provider revision, pending completion,
state delivery or planning edit. `repair_abandoned.py` in the demo directory refuses
the newer journal schema and captures full preimages before commit. The installed-
schema copy passed repair and conditional rollback. Live application then changed
only planning state/revision; the native Work revision trigger handles refresh.
Live before/after counts: 547 Tasks, 564 PRs, 3292 Sessions, 68406 Task events—
unchanged. No new provider requests were created. Backup `live-before-abandoned-repair.db`,
preimages `abandoned-live-before.json`; rollback refuses any subsequently edited row.
Do not restore a whole snapshot over concurrent live activity.

The newer private demo copy used the existing common planning writer for these
same IDs, not direct journal edits. Temporary helper source remains only in the
demo directory (`clone_abandonment_repair.rs`), removed from production source
after its successful invocation (`/tmp/loo462-clone-abandonment-repair.log`).
Native Product Task inventory now reads 24 rather than 65; old LOO-149 etc. are
absent, Workflow renders, and other open Work tabs survive. Provider opening remains
intentionally sandbox-blocked; an Open Task error was canceled without retrying
against a real provider. Remaining plan rows include LOO-299's superseded title,
which is not by itself authority for another cancellation.

## Task click fallback — October 12

Jack Heart expects sidebar Task clicks to open their conversation directly, not
show New shell. Native inspection found an Open Task failure for LOO-299: no
checkout. Its private-copy row is unstarted with no primary Session; the sidebar
also admits nonterminal Tasks with associated Sessions, so visibility alone does
not prove Started. This reachable working-set rule still needs reconciliation;
no additional Task disposition was inferred or changed.

Before mounting a Session, failure left the empty pane offering New shell after
alert dismissal. Task workspaces now retain the error/progress; Retry targets
the Task. Empty Task panes no longer offer shells. Layouts/clients are unchanged.
This repairs presentation, not missing checkouts or sandbox-blocked providers.
The running demo has not been replaced; native continuation remains unproved.

Check: `scripts/test_desktop.sh --filter WorkDestinationTests -Xswiftc -gnone --no-parallel` — PASS, 51 tests; regression retains failure after navigation/dismissal. Earlier builds failed on an optional WorkReference in the inline Retry action; extraction and optional binding fixed it. `git diff --check` — PASS. Native demo remains separate.


## Provider-backed conversation — October 12

Jack Heart authorized a fresh configured-provider conversation in an isolated
checkout, preserving existing Sessions. Separate app: demo directory's
`Loopflow Conversation Demo.app` (`com.loopflow.mac.demo.loo462conversation`),
private `conversation-home`, and private `conversation-tmux` via TMUX_TMPDIR.
`launch_conversation.py` and `conversation_command.py` retain exact launch setup.
The older populated app remains sandboxed; copied live Session identities have
not been connected. No provider secrets were copied or printed.

Offline-clone checkout failed because it lacked main. `get_default_branch` now
uses the current branch only when there are no remotes and no local main; main
and connected-repository behavior stay unchanged. Review kept this fallback
limited to standalone repositories. Check: two focused default-branch tests,
`cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` — PASS.
Task `lf-9154ddb` lives only in the private Home, with its own lf-created checkout.

First Session `311515627f5c4860a9d7f37006cce02a` timed out before provider identity:
Codex was waiting for folder trust inside hidden tmux. Later opens failed with
Previous AgentProcess has no observed exit. Its failure history remains. The
supported session replace operation settled that failed demo provider and created
`f3135941b2d84df9afa164ba2962d937`. Native inspection confirmed folder trust was the
blocker; trust was limited to this authorized disposable clone. An unrelated
UserPromptSubmit hook remained untrusted, not globally enabled. The provider
then greeted Jack and waited. Explicit Move here transferred only this demo
client into Desktop. After skipping the untrusted-hook prompt, the actual native
terminal displayed the retained greeting. Native keyboard input submitted
“Reply only: demo input received”; the provider visibly replied “demo input received”.
This establishes provider-backed Desktop input/reply, not automatic first opening.

Remaining defects: hidden first-use prompts and timeout recovery; initial opening
starts elsewhere and needs Move here; light-mode placeholders are nearly invisible
on the dark terminal background. Failed pane exits obscure launch errors. These
are reachable product failures, not old database debris. No Task completion,
Workflow start, real Session takeover, installation or demo approval is inferred.
