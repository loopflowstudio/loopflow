# LOO-672 — Task conversation demo, November 4, 2026

## Native re-demo — November 4, 2026, 20:00–20:02 PST

**Result: partial; load-time prefetch fails. No acceptance from Jack Heart yet.**
Candidate source `7b23c50c20` (current HEAD adds notes only), private data from
`demo-evidence/automatic-manifest.json`. Launched the prepared candidate with its
matching CLI, explicit repository and private LF_HOME. Changed only its disposable
bundle identifier/display name to `com.loopflow.mac.automatic.loo672` / **Loopflow
LOO-672 Automatic**, then ad-hoc signed it, to distinguish the older running demo.

Observed through native Desktop, without pressing Start/Open:

- Repository load created the absent primary Session for **Automatic conversation**.
  But `automatic-load-ps.json` was empty and all four Session records had null
  agent process IDs. This is preparation, not usable native input.
- Selecting **Automatic conversation** opened the real Codex composer and initial
  reading turn. Selecting **Claude continuity** opened real Claude without folder
  trust or a Start click. The native UI and first turns rendered.
- The untouched **Codex continuity** Task remained idle, with its original Session
  `95e5e441ab32454aa160d7957c5c3bf3`, unknown Session state and no agent process,
  even after the other conversations completed. This fails Jack's load-prefetch
  requirement. Evidence: `automatic-unselected-status.json` and `automatic-current-ps.json`.
- Selecting **Stopped run** opened Claude. Its final report described the failed
  Graph and the recorded “start it” comment, and explicitly did not execute it.
  No Graph was live in the later process read. However, three additional failed
  Graph records appeared at 04:00:22Z, around launch, compared with the preparation
  manifest. A prelaunch read already showed an lf Session process. Therefore this
  is **not** clean proof that opening/load caused no Graph execution. Determine
  the origin of those runs before claiming this acceptance check passes.
- No draft text was entered by the reviewing assistant; no exact-draft, restart,
  Rex takeover, cold-return or memory acceptance is claimed. No Task Graph for
  LOO-672 was restarted; no installation, merge or shipping occurred.

The app log also contains repeated SwiftUI AttributeGraph cycle warnings
(`/tmp/loo672-automatic-app.log`); their relationship to prefetch is unresolved.
The hidden warm-pane mounting and preparation queue are investigation entry
points, not a proven diagnosis. Repair the real rendered prefetch path and repeat
fresh load with provider liveness checked **before any Task selection**. Also make
stopped-run setup free of ship retries and reconcile the extra runs above.

Current review page: `scratch/demo.html`. Native evidence:
`demo-evidence/automatic-selected.png`, `automatic-claude.png`, and
`automatic-stopped.png`. Design remains `loo-672-ready-conversations.md`; the
accepted no-click/load-prefetch direction is unchanged. Disposition: **available**
for review in the open **Loopflow LOO-672 Automatic** app; implementation still
needs the prefetch repair. Earlier sections below are historical preparation and
review evidence, not current acceptance.


## Current candidate: automatic opening — November 4

Candidate for Jack Heart's re-demo of automatic opening, built from a tree identical to
`7b23c50c20` (its `lf` reports `3ef57e857.dirty` only because it was built before that
commit; `git diff HEAD -- rust swift` was empty at preparation).

- App: `/private/tmp/loo-672-redemo-eh8h08q0/Loopflow LOO-672 Redemo.app`
- Launch from a display session: `/private/tmp/loo-672-redemo-eh8h08q0/launch.sh`
- [Manifest](demo-evidence/automatic-manifest.json): repository, Machine, Tasks and checkouts.

Preparation is the only thing exercised so far. Fresh data holds four owned idle Tasks:
**Automatic conversation** (absent Session), **Claude continuity** and **Codex continuity**
(prepared Sessions, no native history), and **Stopped run** (stopped Graph, comment “start it”).
`lf ps` was empty and no process referenced the candidate. No app window or provider has run,
so **automatic opening has not yet been observed in the native app**. That remains the demo's
acceptance, with no Start or Open click: load opens all four; selection and relaunch keep
identity; Stopped run reports its status and the comment, and `lf ps` shows no new Graph.
Drafts, Rex Take back, cold return, first paint against usable input, and the twenty-Task
footprint and first-turn cost also remain unmeasured.

Checks on this tree, headless: `ops::human_session` 37 passed; `context_launch` 15 passed;
`claude_trust` 7 passed; `cargo fmt --check` and clippy with warnings denied clean; Desktop
714 tests in 88 suites passed, in the runs described below. The whole gate has not run.

### Desktop suite crash that depends on environment size

`scripts/test_desktop.sh` failed three of three times here with exit 1, stopping in
`defaultConfigurationLoadsAfterInitialAppearance` at 402 passes with no summary. Breakpad's
minidump puts the fault (EXC_BAD_ACCESS) in `ghostty_config_finalize`. The same binary and
command gave:

| Launch | Result |
| --- | --- |
| Wrapper, filtered to all tests | crash |
| Wrapper's final command in bash, no build, twice | crash, crash |
| Same, one unrelated added variable | crash |
| Same, two added variables | 714 passed |
| Same, three added variables | 714 passed |
| zsh, relative paths | 714 passed |

The test passes alone and within its suite. Only the count of unrelated variables changed
between crash and pass, so this is not a result of this branch's behaviour; the branch adds no
`setenv` and does not touch the test. Inferred, not observed: libghostty holds an `environ`
pointer that an earlier test's `setenv` reallocates. Whether the parent crashes in the same
environment was not tested. Gate may hit this. Not fixed here: a fix belongs in a separate Task
if Jack Heart wants one. Logs: `/tmp/loo672-desktop-full2.log`, `/tmp/loo672-P{1,2,3}.log`.

## Latest review: automatic startup still missing

Jack Heart supplied the Codex continuity screenshot showing **Conversation
unavailable**, **No saved provider history**, and **Start conversation**, then
said: “but the conversation should be auto stated and opened” and “thats teh whole
pointof this task”. The assistant had started the named Codex conversation by
clicking Start while this clarification arrived. That manual setup does not meet
the requirement. The prior Start-button/native-launch observations below remain
valid narrow evidence, but do not establish the intended Task experience.

Working Tasks must automatically create/start/open their primary conversation
when needed on load or selection. Existing history resumes; absent Session and
missing native history are both automatic cases. No Start/Open click. This
supersedes the previous explicit-Start design constraint. Preserve bounded warmth,
prefetch, Close, non-replacing claims and explicit Take back. No Task Graph or
replayed prior input is authorized by navigation. Backlog Tasks remain outside
working-Task prefetch.

The review has not passed. The design and accepted decisions now carry this
clarification. Recommended and authorized correction: implement automatic working
Task initialization, verify both absence cases in the actual native app without
manual setup clicks, then return to demo. No installation or shipping approval
was given. The current candidate still contains the rejected Start gate.


## Interactive re-demo opened — November 4

Jack Heart invoked `demo` again after the correction was published. The assistant
launched the prepared candidate with its private Machine, bundled CLI and explicit
repository argument. The manifest's source diff hash matches published commit
`0acc950924` relative to its recorded `df07fd9b5f` base. No main installation or
registry was modified.

Observed through the native app:

- Selecting **Start a conversation** initially showed Open while the read settled,
  then exposed **Start conversation**. The prior Machine cleanup error was absent.
  The redundant passive-reveal explanation and Retry remained above the pane;
  no design acceptance is inferred from that presentation.
- Clicking Start once launched real Codex 0.166.0 on the disposable Task checkout.
  Its native composer rendered; after startup, the sidebar showed Done. This was
  an explicit Start, not navigation starting a turn. No Graph launch was observed.
- Selecting **Claude continuity** offered Start for the prepared Session with no
  native history. Clicking it launched real Claude, which reached a folder-trust
  prompt for the disposable checkout. The assistant left that prompt for Jack.
  Claude also printed that its fullscreen renderer had previously failed and was
  disabled on this machine; its displayed fallback is retained as evidence.
- The former local-Machine cleanup error was absent on both selected Tasks.
  This is rendered evidence in addition to the earlier real-CLI lifecycle tests.

Current screenshots: [Start](demo-evidence/redemo-start.png),
[Codex launched](demo-evidence/redemo-codex.png), and
[Claude trust prompt](demo-evidence/redemo-claude-trust.png).
[Process observation](demo-evidence/redemo-processes.json) contains native Claude
and Codex agent processes. It does not prove exact draft fidelity or readiness
latency. Jack has not yet provided re-demo feedback or acceptance.

The app is left on **Claude continuity**. First review: inspect/answer the trust
prompt for this disposable checkout, type an unsubmitted draft, switch to
**Start a conversation** (the now-running Codex conversation), and return. Report
whether the exact draft and content remain and whether input is immediate. The
separate **Codex continuity** Task still permits testing explicit Start from
prepared metadata. Rex, cold return, relaunch/prefetch, stale-owner recovery and
twenty-Task private footprint remain to be exercised; this first interaction is
not full acceptance. Record subsequent feedback here and regenerate `demo.html`.

## Current re-demo candidate — November 4

The screenshot corrections and non-replacing Start repair are ready for review.
The matching app and CLI are packaged at
`/private/tmp/loo-672-redemo-qevxy417/Loopflow LOO-672 Redemo.app`.
Run `/private/tmp/loo-672-redemo-qevxy417/launch.sh` from a display session.
The [current manifest](demo-evidence/redemo-manifest.json) records the disposable
repository, Machine, Task/Session checkouts and source identity. Preparation
verified those identities; no provider or app window launched here. The main
installation and registry were untouched. Preparation log:
`/tmp/loo-672-publish-redemo.log`.

The headless Desktop rerun passed all 700 tests, including the production Start
button, local retain/departure/return, fresh-load prefetch and selected-first
restore. The Rust run passed 225 of 226 selected tests; the stale-viewer fixture
failed on an initially unknown viewer flag, was repaired to wait for known
evidence and expect the added Reveal action, then passed in isolation. Prepared
Start contention and explicit Take back passed in the broad run. Clippy,
formatting, architecture, Swift boundaries and 72 website checks passed. Xcode's
fallback configuration also passed `build-for-testing`; its log is
`/tmp/loo-672-publish-xcode.log`. Optional display diagnostics were not run.

The initial 701-test Desktop run exposed outdated assertions for restored
selection, completed departure and disconnected shell retention. Those now check
the accepted behavior. A sleep-based debounce test failed in the full run and
passed unchanged alone; it was removed under the repository's flaky-test rule.
Remaining layout-persistence tests pass. Failed evidence is preserved in
`/tmp/loo-672-publish-rust.log`; the clean Desktop rerun is
`/tmp/loo-672-publish-swift-recheck.log`, with the repaired Rust fixture in
`/tmp/loo-672-publish-stale-viewer.log` and lint in
`/tmp/loo-672-publish-clippy.log`.

Native Claude/Codex input, exact drafts, Rex takeover, reboot recovery and the
twenty-Task private-memory measurements remain interactive re-demo work.
The earlier candidate below is preserved as historical evidence.

## Earlier correction prepared — November 4

The two screenshot defects are repaired in the working tree above `df07fd9b5f`.
The empty Task pane now obtains **Start conversation** from Rust's existing
resume explanation when there is no Session. That candidate carried ordinary resume intent for Start. The later launch repair
above replaces it with non-replacing Start through execution and transport claim;
navigation and Retry retain reveal intent.
Local presence uses the local query; remote presence retains its existing route
and observed Task identity.

Headless proof used an initialized disposable store and the actual candidate CLI.
The production empty-pane button was inspected and clicked after an ordinary
Session refresh: passive selection admitted no Session, Start prepared one on the
correct checkout, and no agent process launched. A completed Task's local
retention acquired a hold, set a future departure deadline, and canceled it on
return without the “is not added” error. The Rust regression also proves absence
supplies Start without creating a Session. This is model/view/CLI proof, not native
terminal or visible-input acceptance.

Candidate: `/private/tmp/loo-672-redemo-q0t_sxh7/Loopflow LOO-672 Redemo.app`.
Launch from a display session with
`/private/tmp/loo-672-redemo-q0t_sxh7/launch.sh`; the launcher pins the matching
repository, private Machine and bundled CLI. It has not been launched here.
[Manifest](demo-evidence/redemo-manifest.json) records Task/Session/checkouts and
source identity. `uv run python scratch/prepare-redemo.py` reproduces this setup
from the current debug CLI and Swift app builds. It creates a new temporary
Machine and app bundle; it does not install or migrate the main environment.

Three working Tasks are available: **Start a conversation** has no Session;
**Claude continuity** and **Codex continuity** have metadata prepared for their
named native providers, with matching Machine/checkout identities and no provider
thread yet. No stand-in provider is used. Provider executables are present, but
credentials, launch, usable input and draft continuity have not been exercised.
Start each explicitly before trying native switching, prefetch and restart.

The first correction setup attempt queried `desktop task` as if it were the plan
DTO and stopped with `KeyError: tasks`; the script now uses `desktop plan --task`.
The failed log remains `/tmp/loo-672-redemo-prepare.log`; successful preparation is
`/tmp/loo-672-redemo-prepare-2.log`. This setup correction did not touch product code.

Earlier correction checks: 9 Rust tests and 5 Desktop tests (8 cases) passed in
`/tmp/loo-672-compress-{rust,viewer,swift,redemo}.log`. The candidate matched that
earlier source; it does not include the launch repair. Current verification and
remaining owners are in the [design](loo-672-ready-conversations.md). The earlier
prepared manifest remains at
`/private/tmp/loo-672-redemo-_s56bosu/manifest-before-compress.json`.

## Earlier failed review

The demonstration failed. Jack Heart supplied a screenshot of **Record a reader
floor per migration** showing “No existing conversation; start one explicitly”,
an Open conversation button, and a Machine cleanup error. Jack supplied no prose
acceptance. The assistant's diagnosis below distinguishes the screenshot from
its interpretation. At that review LOO-672 remained at demo. No Graph had been restarted,
no PR merged, and the installed app and main registry were not upgraded.

Candidate: [PR #1816](https://github.com/loopflowstudio/loopflow/pull/1816),
`e525a2b3cf`. Design: [ready conversations](loo-672-ready-conversations.md),
[decisions](questions.md), [load prefetch](loo-672-load-prefetch.md).
Runbook: [demo.html](demo.html).

## What was observed

1. The candidate CLI refused the existing main registry with a migration-history
   or schema mismatch. It reported candidate `0.15.1+7bc74405f.dirty`, latest known
   `wave_removal`, and the store last migrated by `0.15.1 (ac8e088f5)`.
   This blocks the configured main-store demonstration. No migration was attempted.
2. A separate app bundle was assembled from the already-built candidate Swift
   executable and CLI at `/tmp/loo-672-demo-e525a2b3/Loopflow LOO-672 Demo.app`.
   It uses bundle ID `com.loopflow.mac.demo.loo672` and disposable data.
3. The initial density fixture was unsuitable: its conversations were authored
   before their Task checkouts existed. The UI refused those mismatched locations.
   Launching from the real checkout also produced a repository identity error;
   using the fixture repository as cwd removed that error. This is setup evidence,
   not a demonstrated LOO-672 regression. Preserve the failed screenshot.
4. The existing `portfolio` recipe gives conversations matching their checkouts.
   Launching from `/private/tmp/loo-672-demo-e525a2b3/portfolio/repo` with that
   world's environment showed two working Tasks. Selecting **Open conversations
   faster** opened its terminal without clicking Open. The screen reads
   `resumed ce07138d-109b-4ede-8550-a5e7547b0769: 3 lines`.
5. The portfolio candidate still displays **Workspace cleanup pending: machine
   ... is not added**. Its cause was unresolved during this preview (the local-routing
   defect corrected above); this preview proves no successful retention or cleanup. The preview uses the recipe's shell stand-in for Claude,
   not configured Claude/Codex. It proves only limited rendering/navigation.
6. No real-provider usable-input latency, native draft preservation, Rex takeover,
   reboot recovery, or twenty-Task private-memory budget was demonstrated.
   The earlier automated prefetch tests remain separate evidence.

Screenshots: [installed before](demo-evidence/installed-before.png),
[failed density attempt](demo-evidence/candidate-first-failure.png),
[candidate after selection](demo-evidence/candidate-after.png).
The installed screenshot is an existing repository window, not a matched Task
baseline. It cannot establish a before/after performance or behavior comparison.
The process observation is [portfolio-processes.json](demo-evidence/portfolio-processes.json).

## Earlier stand-in preview (superseded)

The earlier portfolio preview was limited rendering/navigation evidence. Use the
new disposable candidate above for the correction review; the screenshot failures
and stand-in limitations remain recorded here.

## Required configured demonstration

After a supported compatible candidate environment is prepared, use real Claude
and Codex Sessions. Start at a fresh repository load, with no saved visits, and
verify working Tasks prefetch automatically without changing selection. Then:

- A → B → A: type an unsubmitted draft; verify exact draft and provider identity.
- Exceed the warm set, return cold: saved history precedes input readiness and no
  new turn starts. Measure first paint separately from usable input.
- Control a disposable Session in Rex: selection stays passive, last content
  remains, and only **Take back** transfers input. No Fork button.
- Close a pane and wait for refresh: it stays closed until deliberately selected.
- Relaunch the candidate: selected Task returns first, bounded preparation follows.
  Exercise stale-owner/reboot recovery only in disposable data.
- Visit twenty Tasks and measure Desktop + lf + provider private footprint, listing
  visible exceptions and protected providers. Eight is still provisional.

## Screenshot follow-up: original diagnosis before correction

The earlier “try both working Tasks” instruction was unsuitable: the portfolio
recipe intentionally gives **Record a reader floor per migration** a checkout
and Graph history but no Session. That explains why passive reveal refuses;
it does not excuse the missing recovery action.

1. **No Session must offer a usable explicit Start.**
   `WorkspacePanesView.emptyWorkspace` always labels the button Open conversation
   and calls `WorkModel.openTaskConversation`. Its linked preparation always uses
   `.reveal`, and Rust `select_session` refuses when no primary Session exists.
   Repeating that action cannot create the missing conversation. The earlier
   recovery fix covered an existing Session without provider history, missing the
   distinct case of no Session at all. Supply the legal explicit Start action
   through the shared model and carry its intent to preparation; navigation and
   Retry remain passive. Test a real started Task with no Session, ordinary
   refresh, and the actual visible Start click. Preserve takeover protection.
2. **Local retention must reach the local lifecycle owner.**
   `SessionsWorkspace.reportPresence` calls `query.onMachine` unconditionally,
   including when the location equals `localMachineId`. That prepends
   `--machine <local-id>`. CLI `find_machine` searches saved Machines and fails to
   resolve this local identity. A disposable CLI reproduction confirms that
   `machine id --json` reports this ID with route `local`, ordinary `task list`
   succeeds, and the same command with `--machine <that-id>` fails “is not added”.
   Evidence: [local-routing.txt](demo-evidence/local-routing.txt).
   Reuse the local query for local presence, or repair the shared resolver to
   accept its own local identity consistently. Preserve remote routing; do not
   register this Mac as an SSH peer to hide the failure. Verify local retain,
   departure and return through the real CLI in an isolated initialized store.

These were implementation gaps under the accepted design, repaired by the
correction above. No additional product choice was required. The native-provider and footprint proof remain outstanding.

## Correction authorized

Jack Heart then requested: “ok, get it fixed and let me know when ready ofr me to
redemo”. This authorizes the correction pass and preparing a usable re-demo, not
shipping or installation into the main environment. The correction above implements
both repairs and prepares matching repository/Machine/checkout data. Prefetch,
warmth, takeover and Close requirements are unchanged. Gate and correction
publication remain before Jack's interactive re-demo; native proof stays open.
