# Work-first native demo — October 11, 2026

Jack Heart invoked demo and selected real configured repositories. This authorizes
this demonstration, not cleanup or landing. Design: [get-me-using-lf.md](get-me-using-lf.md).
Published review: https://github.com/loopflowstudio/loopflow/pull/1531.

Built revision `0baa92dc16` with the repository development launcher and launched
`~/Applications/Loopflow Dev.app` against the Loopflow repository and normal Machine.
The installed production app was left running. The dev CLI forwards normal-Machine
commands to the installed CLI; this does not install branch CLI/schema changes.
Build and launched app share Mach-O UUID `82458694-00C4-3CFB-99D2-F6978A19DBFC`.
Byte comparison differs after app signing, so it is not a binary-parity assertion.

Check: `uv run python scripts/loopflow-dev.py build` and `run` succeeded; the Dev
app process was observed. Build log: `/tmp/loo462-demo-launch.log`. Existing
Ghostty linker/deprecation warnings remain; process launch proves no usable UI.

## Remaining native acceptance

Repository-name opening must reuse the retained conversation without duplication.
Task opening must show the conversation first with a usable compact Overview;
switching Work must retain unsent input and focus. These observations remain absent.
Empty-repository entry is separate: no real empty repository was created or planning
records removed for this demonstration. No data cleanup or landing occurred.

## Failed real-data opening

Jack Heart supplied a screenshot of the Dev app's Open Work alert:
`error: unrecognized subcommand 'identity'`. The repository conversation did not
open. This is a failed demo, not native acceptance of the workspace changes.

The installed `lf 0.13.11` lacks `repo identity` in `lf repo --help`.
`RegistryQuery.repositoryIdentity` calls `repo identity --json`; invoking that
read through the Dev app's bundled CLI reproduces the same error. The bundled
CLI's `repo identity --help` succeeds because it describes branch syntax, while
ordinary main-Machine execution forwards to the installed CLI. Build parity alone
therefore does not establish runtime capability parity. The demo launcher should
have checked the actual installed command path before asking Jack Heart to try it.

Real configured-repository review is blocked on a compatible installed release
containing the LOO-427 foundation, or an explicitly reviewed supported delivery
path for that foundation. Do not add a fake identity fallback, override the Home,
or bypass release/migration authority to make the screenshot disappear. A private
fixture can test the branch together but cannot satisfy Jack Heart's requested
real-repository demo. No installation of the branch CLI or migration was attempted.

## Next pursuit — Jack Heart, October 11

Jack Heart requested stacking on LOO-427 and continued pursuit through publication
for another demo, not landing. Parent `f1e8cd19ff5b` is integrated at `c040d9fa03`;
both focused WorkCache conflict tests passed. Stacking did not supply the compatible
installed runtime. No live migration, Session takeover or data deletion was authorized.

The continuation at `1787d94bc9`, compressed at `df40c477ef`, repairs `run` and `run-debug` to check repository opening through
the bundled helper's ordinary dispatch before launch, retaining the actual error
and naming the compatible-release dependency. The misleading claim that bundled
source always reads branch data is removed. This operation can register a repository;
it is not a read-only capability probe. Headless executable fixtures cover installed
support/refusal and explicit private-Machine launch. No new real-data demo was run.


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
