# LOO-672 — Task conversation demo, November 4, 2026

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
