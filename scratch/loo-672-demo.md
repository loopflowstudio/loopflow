# LOO-672 — Task conversation demo, November 4, 2026

## Review result

The demonstration failed. Jack Heart supplied a screenshot of **Record a reader
floor per migration** showing “No existing conversation; start one explicitly”,
an Open conversation button, and a Machine cleanup error. Jack supplied no prose
acceptance. The assistant's diagnosis below distinguishes the screenshot from
its interpretation. LOO-672 remains at demo. No Graph was restarted,
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
   ... is not added**. Its cause is unresolved; do not infer that retention or
   cleanup succeeded. The preview uses the recipe's shell stand-in for Claude,
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

## Hands-on review available now

In **Loopflow LOO-672 Demo**, select the two working Tasks and return to
**Open conversations faster**. The expected result is automatic reveal and
retained content, without an Open click. The stand-in provider is deliberately
not evidence for native input/draft behavior. Report the observed transitions and
any unexpected prompt; the persistent cleanup error is already recorded.

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

## Recommended next action

Correct the two reproduced defects below, prepare a compatible configured demo
environment, then repeat the native walkthrough and memory measurement. This is
an assistant recommendation, not a new agreed design change or authorization to
install/merge. Jack's accepted product decisions remain unchanged. Further
implementation belongs to the caller's next choice; this demo did not launch it.

## Screenshot follow-up: confirmed remaining work

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

These are implementation gaps under the accepted design. No additional product
choice is required. The native-provider and footprint proof remain outstanding.

## Correction authorized

Jack Heart then requested: “ok, get it fixed and let me know when ready ofr me to
redemo”. This authorizes the correction pass and preparing a usable re-demo, not
shipping or installation into the main environment. The next pass must fix both
defects, verify their actual paths, and update this record and `scratch/demo.html`.
Keep previous failures as dated evidence. Prepare a reproducible candidate with
matching repository/Machine/checkout identity; do not present a mismatched fixture
or a stand-in provider as configured native proof. Preserve the prefetch-on-load,
warmth, takeover and Close requirements. Stop at demo for Jack's review.
