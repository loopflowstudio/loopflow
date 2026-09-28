# 5 Whys: Sessions resume conversations that were never saved

## The Problem

Jack Heart could not open three review Sessions because Loopflow repeatedly
asked Claude to resume allocated conversation IDs with no saved native history.

## Chain

Open fails → nonexistent native history → allocated ID treated as observed
history → publication and recovery trust the same receipt → tests certify
Loopflow bookkeeping → resumability lacks proof at the provider boundary.

**Problem**: The original LOO-330 open command exits with `No conversation found
with session ID`; LOO-331 and LOO-329 have the same missing-history condition.
CLI and desktop converge on the same `lf session open` command.

**Why 1**: Open chooses native resume for a Run whose Claude conversation was
never found in its selected account. `resume_native_run` reads
`provider-session.json` and calls `resume_session_with_env`, which builds
`claude --resume <id>`. The provider error propagates; it does not return the
`false` that lets `open_boundary` prepare a replacement.

↳ Earlier detection belongs in opening: distinguish confirmed absent history
from existing history and an inability to inspect it. Recover the absent case
through the existing boundary replacement path; do not retry every resume
failure as a fresh conversation.

**Why 2**: The receipt proves allocation, not persistence. `launch_prompt`
derives a Claude UUID from the Run ID. `session_command_status_with_env` writes
that ID and its account before `process.spawn()`. After exit, `launch_prompt`
reads the same receipt and emits `ProviderSessionObserved`, without independent
provider evidence. A successful process exit records `completed` even when
no conversation was saved. All three incident Runs have precisely this shape.

↳ Keep the intended ID and account durable for routing and correlation, but
do not promote them to observed history merely by reading them back. Process
completion and a conversation that can be reopened are distinct outcomes.

**Why 3**: Startup and reopening share this false meaning of history.
`session_run_is_resumable` requires only the receipt and an owned live client.
`spawn_session_run` can therefore release startup before Claude persists any
history. Later, the remaining receipt makes `resume_native_run` take the resume
path even after that client disappears. Recovery already exists for a consumed
Run *without* a receipt; the prematurely written receipt bypasses it.

↳ Use provider persistence evidence for resumability. Keep a pending boundary
visible and openable while startup is incomplete. Recovery must preserve its
identity and publish a prepared replacement before exposing the new binding.

**Why 4**: Existing proofs model Loopflow's records as the provider outcome.
`initial_session_publication_requires_history_and_an_owned_client` writes an
arbitrary provider ID, registers the test process as its client, and asserts
resumability without native history. The CLI replacement test removes the
`prepared` marker but leaves no provider receipt, so it misses the incident's
critical combination: receipt present, history absent. The vendor-launch check
script explicitly checks command and URL shapes, which cannot establish that
the resulting conversation reopens.

↳ Add a behavioral proof at that boundary: a first launch exits before saving
history; the same Session open command then starts with its original context,
and a later open resumes persisted history. Keep command-shape tests for their
narrow purpose.

**Why 5 (Root)**: The Session lifecycle uses one reference to represent both
Loopflow's intention to start a conversation and the provider's durable result.
Names such as “observed,” “history,” and “resumable” promise more than their
evidence establishes. The tests inherit that model, so they confirm the same
mistake. The systemic correction is to make persistence at the provider
boundary determine resumability, and let incomplete startup remain recoverable.
It does not require a new Session identity or another operator approval.

## Evidence checked in this analysis — 2026-09-28

- Re-read all three original Run manifests, terminal receipts, provider
  references, and event types. Each is a Claude TUI Run with outcome `completed`
  and exactly `user_input`, `provider_account_selected`, and
  `provider_session_observed` events. This does not establish why Claude exited
  or prove that no provider-side work occurred.
- Every original artifact hash matches `session-opening-proof.json`. Exact-ID
  searches in the recorded account's native projects directory found no
  original conversation files. All three restored history files still exist.
  Actual launch/resume proof is from the preceding restore step below; this
  analysis did not launch another review or supply feedback.
- Compared the relevant source against installed revision `280670217`: changes
  since that revision do not alter the allocation, publication, or resume paths
  identified here.
- Source owners: `rust/loopflow/src/lf/commands/run.rs::launch_prompt`,
  `lf/commands/util.rs::session_command_status_with_env` and
  `build_resume_session_command`, `ops/human_session.rs::spawn_session_run`,
  `session_run_is_resumable`, `resume_native_run`, and `open_boundary` (all under
  `rust/loopflow/src/`), plus `run_record.rs::read_provider_session`.
- Test evidence: `ops/human_session.rs` publication test,
  `rust/loopflow/tests/session_cli_tests.rs` consumed-launch replacement case,
  and `scripts/check_vendor_session_launch.py`. Desktop launches
  `surface.openArgv` in `swift/LoopflowMac/Views/SessionsView.swift`.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
| --- | --- | --- |
| First launch | Why did all three original Claude clients exit before history existed? Their native terminal output was not captured; do not assign this to auth, EOF, cancellation, or a crash without evidence. | High |
| Persistence | What native evidence is sufficient for the installed Claude version to resume, including delayed writes and an empty conversation? Verify in an isolated provider account before choosing the persistence signal. | High |
| Ownership | Can moving an actively starting client terminate it before history persists? `resume_native_run` stops owned clients before reading the receipt; this is a code-derived risk, not an established cause of these incidents. | High |
| Other providers | Do Codex hooks and OpenCode session-created logs establish durable history or only identity? Their publication mechanisms differ; this incident proves the Claude case. | Medium |
| Desktop | Does the restored command render and accept input in the desktop terminal? No rendering environment was available. | Medium |

## Fixes

| Level | Fix | Prevents |
| --- | --- | --- |
| Immediate — completed | Rebind the three waiting boundaries to prepared replacement Runs and verify actual open/resume; preserve originals and all review state. | These stranded reviews |
| Structural — next | Separate allocated identity from resumable history; recover confirmed missing Claude history through the existing opening operation. Apply to old receipts as well as new launches. | A first launch with no saved history permanently stranding its boundary |
| Systemic | Define Session acceptance as opening with context and reopening saved native history. Exercise premature exit and client handoff at the provider boundary; retain useful startup-failure diagnostics in the Run's private artifacts. | Internal receipts being mistaken for completed external effects |

## Changes to Implement

- [ ] Implement one coherent Claude prevention spanning publication and reopen:
  retain the intended ID/account, establish native persistence independently,
  and let explicit open recover a consumed Run with confirmed absent history.
  Existing receipts must benefit without a manual database repair. Reuse the
  current boundary replacement mechanism rather than adding a parallel one.
- [ ] Preserve boundary identity, name, prompt/context, account routing, cursor,
  invocation, readiness, feedback, failure, and claim during recovery. Keep old
  Run evidence. Prepare the replacement before publishing its binding under the
  existing ownership/position controls; list must never see an unprepared gap.
  Audit the current unconditional `ready_summary = None` in replacement before
  extending that path to this condition.
- [ ] Add focused regression proof for receipt-present/history-absent launches
  exiting both zero and nonzero, followed by successful open with context and
  subsequent resume. Include existing-history preservation and a concurrent
  opener so recovery cannot discard a conversation or launch duplicate clients.
  An inaccessible account/history location remains unresolved, not “absent.”
- [ ] Verify the real Claude path with an isolated disposable conversation:
  premature exit → same Session opens with its seed → saved history reopens.
  Assert no review completion or Flow advancement. Fake-provider tests prove
  Loopflow behavior; label them as simulations, not native persistence proof.
- [ ] In a follow-on diagnostic slice, retain bounded, private startup failure
  evidence and report the observed failure. Avoid the current generic ownership
  advice for all nonzero exits. Do not log credentials or entire environments.

### Review findings

The smallest useful prevention changes the meaning consumed by both publication
and opening. Moving the existing receipt write later, checking only exit status,
adding `--replace`, or automatically restarting on any provider error would leave
the contract broken or risk losing real history. A new retry service, DTO state
machine, or manual repair command is not justified by this incident. Scope the
first implementation to Claude and reuse existing account/ownership mechanisms.

This pass changes analysis only. No production code, installed binaries,
authentication, Session bindings, or review feedback was changed. Source tests
were inspected; no test suite was rerun for this documentation-only work.

## Prior restoration evidence

Jack Heart reported that Sessions would not open from either the CLI or desktop.

## Finish line

The installed `lf session open` command for the reported review reaches Claude
with its review context and can reopen saved native history. The desktop uses
the same `open_argv`; terminal verification alone does not prove desktop rendering.
Listing a Session or preparing a Run does not count as restoration.

## Observations — 2026-09-28

- Installed CLI: `0.12.23+280670217`; data directory:
  `/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea`.
- Reproduced the clipboard command in a PTY. Claude exits 1 with
  `No conversation found with session ID: 9927fb1a-a16a-464b-8800-2c975f5ac3ac`.
- LOO-330, LOO-331 and LOO-329 review Runs each contain a preallocated Claude
  ID, an initial input, and a completed terminal receipt. None has a matching
  native conversation file in its selected account. Their preceding kickoff
  conversation files exist and finish just before review Run creation.
- Launch code writes the allocated Claude ID before spawning; on exit it
  records that ID as observed. Resume trusts that receipt. An initial client
  exit without persisted history therefore strands this review.
- No readiness or completed review feedback exists for these three Sessions.

## Hypotheses

- First launch exited before its initial prompt produced saved history. The
  exact reason is still unknown; there is no captured native terminal output.

## Recovery and live proof

All three review Sessions now open and resume using their original installed
`lf session open '<boundary-id>'` command. Each first launch received its
assembled review context, wrote native user/assistant history, was interrupted
with Escape and exited with `/exit`. A second invocation displayed that saved
history and exited 0. No review feedback was supplied or marked ready.

| Task | Original Run | Restored Run |
| --- | --- | --- |
| LOO-330 | `run_9927fb1aa16a464b88002c975f5ac3ac` | `run_9186b660fa304b208b1dfb946dd870f9` |
| LOO-331 | `run_10afb60a023943dfa0fa45ee15fe6afa` | `run_e44c6e1f2acb4dc3986f0b6c6ffd5a2c` |
| LOO-329 | `run_2e5cd1be2dfa4b9190c999d294cba971` | `run_7b98c4ea500f41ba8b6103111b93a7c3` |

The exact original `task_flow_positions` rows are preserved privately under
`/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea/recovery/session-opening-20260928/`.
Recovery used a SQLite transaction comparing each exact old Run and position
version, clearing only `session_run_id` and incrementing `position_version`.
Installed `lf session open` then prepared and launched the replacement Run.
Final row comparison confirms those are the only two changed columns: invocation,
cursor, review, readiness, failure and claim remain unchanged.

Original Run manifests, inputs, provider receipts and terminal receipts remain
in place. After rebinding, the obsolete Runs appeared as independent Sessions;
`lf session complete <old-run-id>` retired only those orphan surfaces through
the supported command. This did not complete any Task review or advance a Flow.

[Proof receipt](session-opening-proof.json) records native history locations,
exact open commands, message counts and original artifact hashes. A copy lives
beside the private row backups. No binaries, authentication or provider settings
were changed. Desktop rendering was not exercised in this headless environment;
the verified commands are the same `open_argv` emitted for the desktop.

## Review findings and remaining work

- Clearing an unresumable binding without immediately preparing its replacement
  briefly makes the installed Session list fail on an unprepared review. This
  occurred for LOO-329 during recovery and was immediately repaired with
  `lf session open '<boundary-id>' --json`, followed by real launch/resume proof.
  All three are prepared and list normally now. Keep any future recovery atomic
  across replacement preparation and binding publication.
- Native clients received their real review prompts during verification. They
  performed initial reads; the proof interrupted them instead of supplying
  invented review feedback. No proof client remains active.
- The original reason for exiting before native history existed remains unknown.
  Preventing premature publication and recovering this condition automatically
  belong to the following 5 Whys step. No production code changed in restore.
