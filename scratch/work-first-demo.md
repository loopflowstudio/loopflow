# Work-first native demo

Jack Heart requested real configured-repository review, main Work views before
Portfolio. Initial installed-CLI mismatch, failed launch and stacking history:
`ce367d7143:scratch/work-first-demo.md`, opening sections. Those failures remain
unaccepted; no live installation or landing is authorized. Current design:
[get-me-using-lf.md](get-me-using-lf.md), PR #1531.

Empty-clone launch evidence: `53c4c156e3:scratch/work-first-demo.md`,
**Isolated branch demo**. Jack Heart rejected it for populated Loopflow Work.

## Review priority

Jack Heart requested real populated Loopflow Work before Portfolio, no added
chrome or Session counts. Earlier failed launches, redraw freeze, SQL loading
repair, measurements and screenshots remain at
`946dae6203:scratch/work-first-demo.md`, **Review priority — October 11**.
The shared Flow membership query now restricts candidate Processes before joining
history. Its five passing Rust tests prove membership retention, not native speed.
No previous launch or screenshot established real-provider continuation.

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

Native follow-up: typed an unsent “retained demo draft”, selected the Wave, then
clicked the sidebar Task. The same provider conversation and exact draft returned
without New shell or another launch. Left this isolated demo open for Jack.

## Correct repository and Started membership — October 12

Jack Heart's latest screenshot shows the isolated one-Task `loopflow-demo`, not
Loopflow's Task inventory. Raising that window was the wrong demonstration.
The configured-provider fixture proves input/retention only; the target remains
the populated Loopflow repo with correct Task-to-conversation opening.

Jack Heart confirmed installed lf 0.13.12; `lf --version` agrees. An upgrade
is not an established prerequisite. The previous mixed-runtime launch and its
slow-reader evidence remain at `e276eee8a2:scratch/work-first-demo.md`; the live app failed wire decoding, not repository identity. A source CLI
with ordinary Home dispatches to installed lf by design. Private copied data
runs branch code, but its copied execution records grant no live-client authority.

Removed the reachable Session-presence shortcut that admitted unstarted Tasks
into the sidebar. Recorded Started plus shared unresolved execution now owns
membership; retained Sessions and Wave plans remain intact. Review confirmed
this removes a second inference of Started without mutating historical records.
Check: `scripts/test_desktop.sh --filter WorkNavigationTests -Xswiftc -gnone --no-parallel` — PASS, 37 tests; diff check PASS. Native correction initially still loaded; later copied-data rendering below supersedes it. No live installation authorized.

Release alignment `6f9bd12f85` merged main; conflict resolution preserves collapsed
Overview and TaskDueDateView. Focused Swift 45/build PASS. Earlier sync attempts:
`e276eee8a2:scratch/work-first-demo.md`. No push/install.

The synced build still refused the copy's missing `linear_team_checks` draft.
Applying that exact CREATE TABLE to the disposable copy preserved every existing
table's row count; branch identity and Work snapshot then succeeded (21.66 s).
This is fixture schema preparation, not a supported live promotion receipt.
Evidence: demo directory `compatibility-{check,after-sync,with-draft}.log`,
`compatibility-fixture-counts.json`, `compatibility-work.json`.
Snapshot: 11 Tasks, not 12: Growth 312; Infrastructure 367 and lf-9c03797;
Product 462/427/382/380/378/371/330; Spend 389; Intelligence none. Missing checkout,
merged-but-uncompleted, retained dirty files and unsettled commits explain those
inclusions. That explains the rule, not Jack Heart's approval of every remaining
Task. Native clicks and installation remain unproved; no live schema change.
Stopped only the owned orphan reader from the closed demo and redundant one-shot;
the current app reader remains live. Task automation disabled; app CI watcher active.

October 12 copied-data review renders all 11 expected Task rows,
without Session counts. Clicking LOO-462 selects exact primary Session
`955ff6f0a65047a18b191505e307fce1`, showing its breadcrumb and Open here action,
not New shell. The sandbox blocks provider use; placeholder text is nearly
invisible on the dark pane. No provider continuation is proved.
The live-data app instead ended its long read with a wire-decoding error and
retained an older, larger roadmap; it must not be presented as current.

Correction to the runtime diagnosis: Portfolio's root-json error also reproduces
on the branch, not only the installed CLI. Argument ownership wrongly excluded
all global flags below root, including globals declared by Portfolio. It now
excludes inherited IDs only, retaining each nested global at its declaring owner.
Review: shared Clap metadata remains the sole flag owner, with no command-name
allowlist. Check: binary reorder_args tests and portfolio_cli_tests PASS;
`cargo fmt`, `cargo clippy --all-targets -- -D warnings`, diff check PASS.
Repackaged: native inspection shows the 11-row sidebar with the Portfolio error
gone; direct packaged `portfolio --json` exits 0. No live install or provider proof.

LOO-312 fails with “no checkout to hold a conversation.” The copied record has no primary or directly bound Sessions;
this is not a wrong-conversation fallback. Keep it unresolved rather than inventing
history or hiding the Task. Corrected the participant read to `config user --json`
The first placeholder dark-appearance change failed native review: text remained
near-black. Explicit TerminalPalette inks fix it; native relaunch confirms readable text/button.
Check: RegistryQuery/WorkDestination PASS (68); palette repair WorkDestination PASS (51); configured participant read and diff check PASS.
Native LOO-462 click selected its exact Session, then sandbox denied launcher
inspection. Closing that failed terminal exposed the contrast defect. No takeover.

## Live-reader diagnosis — October 12

v0.13.12 shares WorkFrame/wire models; Portfolio is new. LocalLineObservation
now reports decoding paths; the wrong-type regression identifies `sequence`. Check: WorkObservationTransportTests PASS (8); diff check PASS.
Installed read `/tmp/loo462-live-wire/` finished in 690 s: both live frames decode
with current Swift, as do 100 Session records. Earlier live app predates main sync; compatibility is not usable opening.
Jack Heart requested this app on the real database. After unlock, launched
`/private/tmp/loo462-native.FgtDmE/staged/Loopflow Work Demo.app` (PID 26326),
ordinary Home/installed dispatch, actual Loopflow repo. Native view is loading;
Portfolio reports root-json failure. Reader 26587 is live. No migration/takeover.

Live output selects 17 Loopflow Tasks under the current Swift predicate; the branch
snapshot selects 11. Six extra completed Tasks have only old blocked Flow evidence:
LOO-440/412, lf-b978097/df7e403/cb9012a/6f396c0. `c229a29648` fixes that shared
Rust projection; Desktop decoding compatibility does not deliver this unreleased
behavior. Do not delete failed history or duplicate the fix in Swift.

## Bundled runtime — October 12, in progress

Jack Heart authorized the changes needed in this diff for a viable real-data demo,
including bundled lf selection and its propagation into embedded terminals/Session
children. This supersedes the installed-release-only demo hold. Desktop selects
its own helper via LF_BIN; ordinary terminal defaults remain installed. Remove the
external helper override and provenance-only store refusal, not schema validation
or the exclusive migration boundary. Reattachment keeps an already-running
provider's environment; it does not silently restart it for a new executable.

Source includes main's unreleased `linear_team_checks` table. Real data has the
released schema; no live table was added. Its Linear acquisition path and Flow
schema revalidation still need composed proof before claiming all operations work.
No global install, migration, provider takeover or additional cleanup performed.

Check: bundled selection/shared-store Rust 7 PASS (initial over-specific filter ran zero, corrected); Desktop launcher/zsh 9 PASS; Python launcher 8 PASS; fmt, all-target Clippy, Ruff/diff PASS. Native preparation fixture remains display-only.
Live bundled app PID 8749 finished refreshing with 12 rows (new CI Task since the
11-row snapshot); Portfolio error gone. Clicking LOO-462 launched exact
`session connect 955ff6f0a65047a18b191505e307fce1`; provider shows hook-trust review,
left for Jack. His LOO-312 click reproduces missing-checkout opening failure;
restoration sequencing and duplicate error UI remain. No schema migration.
