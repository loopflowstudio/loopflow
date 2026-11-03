# LOO-709: Idle conversations preserve unfinished interaction

## Accepted outcome — Jack Heart, 2026-11-03

Retain this Task, checkout and PR #1804. Jack Heart selected concept-review
**1a and 2a**: preserve editable unsent drafts and the same answerable pending
approval/question. Shipping after implementation and required checks is authorized.
Prior unresolved-choice and no-shipping language is superseded. The authored
code Graph owns the remaining demo, gate, publication, landing and verification.

- Detached, fully settled interactive Sessions end after
  `session.end_detached_after` (default 2m; `off` supported), measured with
  `SystemTime`. Viewer changes, output and held work restart the grace.
- Active/silent native turns, owned watches/scheduled work, pending requests,
  running Graph ancestry or captured-step membership, and headless Runs hold the
  owner. Finished Graphs and stale unviewed terminal status do not. Store failures
  retry. Executable removal/replacement/supersession uses two seconds of output
  silence and the same protection, including while viewed.
- Jack Heart clarified on 2026-11-03 that long Working turns, including quiet
  build/test/CI waits and Task Session supervision of a running Graph, remain
  active until completion or a concrete reported blocker. Recent interaction is
  not required. No sleeping supervisor or new wake-up scheduler is requested.
- Open Rex Task Session tabs retain viewers when another tab is selected;
  Jack accepted this boundary. Visibility integration is outside this Task.
- Save the last screen before normal/policy exit using LOO-710's integrated
  `snapshot::leave`, `$LF_HOME/screens/<session>.ghostsnp`, and
  `snapshot::last_screen` reader. No DTO field or expiry/deletion policy.
  A preview is not an editable draft or an acknowledgement that input is ready.
- Ordinary single-Session resume returns to provider history without replay or
  starting a turn. Delete bulk resume and its Desktop startup caller. Preserve
  pane-specific resume and shared rollover helpers.
- Workspace launch/restore belongs to existing Product Task **LOO-498**, refined
  as “Open and restore a Loopflow workspace in Rex”: Rex first, Herder second.
  No workspace implementation or duplicate Task here. LOO-708 owns process
  overhead; LOO-705's monitor rollover owns the 27 old pre-policy owners.

## Current implementation and input authority — reconciled 2026-11-03

Upstream main `2bc29a72e0` integrates LOO-705 rollover, LOO-708 launcher/relay
reduction, LOO-710 Ghostty snapshots and LOO-712 viewer byte-resume. This checkout
is synced to that base. A queued `Ending` candidate becomes `Event::Control`
only after confirmation, preserving the successor command. No parallel snapshot
or viewer protocol is needed.
The existing silent-turn/watch repair and Ghostty integration remain intact.
Pending input now precedes viewed terminal-idle evidence in `TURN_LIVE`.
Retained-provider attachment handoff carries its current activity reading to the
new token; stale writers cannot settle it and replaced providers cannot hold it.

Jack Heart's investigation steer requested a conservative input ledger rather
than treating missing editor APIs as an immediate blocker. The current design
uses **one nullable `processes.native_input` column on the existing terminal
owner**, with one unreleased migration, `native_terminal_input.sql`. This revises
the earlier no-schema approach: all three ending callers (policy, resume takeover,
monitor rollover) already ask the shared owner judgment and must see the same
input protection. There is no second Session lifecycle or editor restoration API.

The owner records terminal input before forwarding it. The ledger observes plain
text and a single submission candidate; Enter alone cannot clear it. A fenced
native Claude `UserPromptSubmit` must match the exact candidate text and name a
`prompt_id`; the matching Stop can then clear it only if no intervening input or
ambiguity remains. Failed/rejected prompts without settlement retain their owner.
Native completion explicitly clears the old editor. SQLite errors withhold input
for retry rather than forwarding an unrecorded key. The serving owner rechecks
pending input, store state and attached viewers before honoring a queued ending;
new viewers veto idle ending but not executable rollover.

**Coverage and conservative retention:** only native Claude callbacks with the
observed `prompt_id` can acknowledge this plain single-submission path. Native
editor state has no general checkpoint API in the inspected hooks/status line.
Backspace, paste, long input, other unknown controls, input during a turn, multiple submissions,
missing IDs, or nonmatching acknowledgements retain the owner until native exit.
Other native providers retain typed input because they have no equivalent ledger
acknowledgement here. Do not imply every settled edited/pasted Session ends.
Never-typed, otherwise settled Sessions retain the prior idle-ending behavior.
Complete known terminal replies are excluded from editor input; arbitrary escape
sequences are protected. A terminal reply split across acknowledgement/settlement
can finish afterward without preventing release; incomplete/unknown input still holds.

Local Claude Code 2.1.317's prior watch probe recorded the same `prompt_id` on
UserPromptSubmit and Stop, including background notifications, at
`/private/tmp/native-watch-y1t6wzw1/hooks.jsonl`. The [hook reference](https://code.claude.com/docs/en/hooks#userpromptsubmit)
says submission hooks run before processing, may reject input, and also run for
scheduled/background messages. Escape during a prompt check can restore its text.
Consequently a submission callback alone is insufficient; matching settlement,
input ordering and conservative handling of ambiguity are required. These source
facts do not replace the editable-draft demonstration.

## Delete — do not maintain

Completed cut removed `SessionCommand::Resume.all`, its dispatch/routes/explanation,
`restore::{interrupted,resume_all,render}`, `unexited_session_owners`, the
bulk-only transport test, current usage docs, `RegistryQueryLocal.resumeInterrupted`
and its app startup call. Preserve `restore::{resume,describe,ResumeOutcome,
ResumedSession}` for rollover and its serialized sweep. Published historical
release notes retain their dated description. Single-Session/pane resume remains;
no deletion targets remain in this cut. The ending-race repair replaces the bare
`Event` candidate and pre-drain `may_end` check with one revision-bearing candidate;
the old path and terminal-idle precedence over unyielded turns are removed.
Candidates now carry an explicit executable change (or idle expiry), rather than
encoding policy in a viewer event. `Ending::confirm` owns the final activity/store
check and constructs the notice only on success, so vetoed candidates no longer
log an exit. `TURN_LIVE` directly accepts either fenced provider work or viewed
working status; neither source can negate the other's protection.

The input ledger now represents submitted text and acknowledged prompt IDs as
separate states. Matching acknowledgement discards the text; only matching Stop
can release the ID, and later input still makes the editor uncertain. Transport
records directly from its pending buffer before writing; the recorded prefix
still prevents partial-write retries from recording input twice.

Prior completed cuts removed the `.vt` snapshot writer/helpers, `LONGEST_PAUSE`,
`recheck`, `held_until`, `Grace::due`, duplicate `RUNNING_SESSION_GRAPH`, store-only
executable-gone judgment, unreadable-config disabling, UserPromptSubmit's steer
shortcut, native Stop's unconditional work clearing and terminal-idle precedence
over owned work. Do not reintroduce them or a parallel input/lifetime authority.

## Concept review repair — 2026-11-03

Jack Heart's accepted experience remains: detach without losing unfinished
interaction; return to the same editor/request while it exists, or reopen settled
history without replay. The input ledger retains an owner; it is not a recoverable
editor. Ghostty preserves display, not readiness or request authority. No new
product choice or scheduler is needed. LOO-498 retains workspace restoration.

The reviewed source defects were a queued ending consumed before ready output,
and a viewed idle report taking precedence over a fenced unyielded turn. The
serving owner now drains output before judging an ending candidate. Each candidate
carries the output/viewer revisions the policy observed; every read advances the
output revision, including native bytes temporarily held from display. New or
unreported output invalidates either policy's candidate and flushes current
activity back to restart its quiet interval. Idle endings also require unchanged
viewer history and no current viewer; executable rollover ignores viewer changes.
Pending input and the shared store judgment still veto ending. Repeated identical
activity does not extend quiet or create a retry loop.

`TURN_LIVE` now prioritizes current-attachment unyielded work alongside open tools
and pending requests, before viewed terminal status. The handoff regression keeps
an unyielded turn with zero tools/input despite viewed idle status, then proves
settlement and provider replacement fences. The native transport fixture survives
both deadlines, including viewed executable removal, then survives native
settlement while its Graph remains live and ends after the Graph completes.
This adds no authority from Task membership.

Focused transport regressions force queued output, read-but-unconsumed activity,
throttled output and queue/attach ordering through the real serve loop in isolated
test processes. Both ending policies retain the updated screen. The policy clock
regression checks that fresh output restarts both intervals and subsequent quiet
permits ending. These checks use internal policy channels and synthetic provider
streams; they establish no configured/installed or twenty-return acceptance.

The pre-repair uncommitted review is retained in
`.lf/tmp/loo-709-before-ending-repair.patch`. Existing draft, same-request, watch,
attachment, Graph and snapshot protections remain on their original paths.

Source reconciliation at `74880b72e3` confirms both requested repairs and their
focused regressions are present. No further implementation mismatch was found
in the reviewed ending/turn paths. The remaining obligations below are acceptance
and delivery, not an unresolved product decision.

Headless concept review at that same HEAD leaves the accepted design unchanged.
The normal path retains the editor/request until settlement; failure to read the
store retains execution for retry; return attaches to that owner or resumes ended
provider history without replay. The queued-output/new-viewer and viewed-idle/
unyielded-turn counterexamples above now have explicit final-judgment protection.
Keep Session identity, provider activity and terminal preview distinct: no new
user action, lifecycle record or scheduler is needed. The existing usage guidance
already expresses these boundaries. This review inspected source and recorded
proof only; it adds no runtime or installed acceptance. No open product choice
and no next implementation direction remain; loop-or-next receives no change.

## Remaining acceptance and delivery

1. Gate completed on 2026-11-03, including input-ledger, silent-turn, watch,
   Graph/headless, rejected prompt/startup and queued-ending regressions. The
   gate repaired retry loss after failed viewer writes and repository reads;
   results and fixture corrections are recorded below.
2. Configured native startup and slow/failed resume are demonstrated in the
   [2026-11-03 headless demo](loo-709-idle-demo.md). Verify the merged installed
   build during delivery; the shared installed app remains unchanged.
3. Twenty fresh configured conversations completed end/return/end cycles with
   unchanged native messages in that demo. This supersedes the three-return
   acceptance gap; the older cohort remains historical evidence, not twenty
   completed returns. The new cohort was sequential, with a 4s test grace.
4. The affected Rust checks, headless Desktop build/tests, website checks,
   migration checks, disposable installation/upgrade proofs and macOS/Linux
   Clippy have passed. CI owns the full matrix; optional display diagnostics
   were skipped. Neither source checks nor disposable Linux promotion establish
   merged installed macOS acceptance.
5. Update PR copy with actual proof and limitations; publish, land, verify merged
   behavior and complete the Task through the authored Graph. No further shipping
   permission is required. Do not merge without editable-draft proof.

## Native input proof — 2026-11-03

Branch debug CLI, real Claude Code **2.1.317**, a disposable Home/repository and
credential-free local Messages fixture, 4s detached grace:

- Typed `NATIVE_DRAFT_X`, detached for 7s, removed the owner's executable, waited
  another 4s, reattached to the same owner, backspaced `X` and submitted `Y`.
  The model received `NATIVE_DRAFT_Y`; no draft replay or replacement provider.
- A separate `SINGLE_PLAIN_PROMPT` matched UserPromptSubmit/Stop IDs, settled,
  then its owner ended after detachment.
- A native `AskUserQuestion` remained answerable after the same two deadlines;
  returning answered “Keep” on tool-use ID `pending-native`. The model received
  the result for that exact request.

Seven model requests; all three owners and their recorded processes were gone
after cleanup. Probe: `.lf/tmp/native-input/probe.py`; evidence:
`/private/tmp/native-input-hvcwttso/` (hook payloads and terminal output).
The first probe at `/private/tmp/native-input-d89f2bz3/` preserved the draft but
retained a plain submission; investigation found the split-terminal-reply
settlement hole and added a deterministic regression. A subsequent instrumented
plain probe and the complete probe above passed. This is real native editor/tool
behavior with synthetic model responses, not installed acceptance or all twenty returns.

## Prior evidence retained

At the branch debug build on 2026-11-02, real Claude Code 2.1.316 in a disposable
Home: 20 conversations detached at 13:53:07; all 60 processes (20 owners,
20 launchers, 20 Claude), 9.9 GB RSS, gone by 13:55:23. Twenty screen files and
idle-ending log lines; no store errors. Three resumes used provider history
without replay. The 27 old owners were untouched. Installation and the remaining
seventeen returns were not proved.

The 2.1.317 local Messages-fixture watch probe observed one running shell task
at Stop; releasing its completion file produced a second Stop with empty task
and cron arrays. Existing focused regressions cover missing snapshots, scheduled
work, child isolation, handoff/replacement fences, native silent turns, output,
Graph membership, policy snapshots and installed symlink changes. Source integration
with LOO-710 is complete at main `ece53eec1b0274bd549525be5efbdc0bfb3965f2`.

Longer prior evidence/design: `42552d07c70d453836b548e943a4fe87356006eb:scratch/loo-709-idle-sessions-end.md`.
Reconciliation read Infrastructure's full memory and its sole immediate child,
Release's GOAL/MEMORY. Release's distinction between source proof and installed
acceptance still applies; no child-specific execution history moved to the parent.
Older parent planning/recovery detail remains at
`39f57361dd597044c0939d0f3ca1aa67d223ea6a:.lf/wave/infrastructure/MEMORY.md`.
No product choice remains unresolved; the conservative retention limits above are
explicit implementation coverage, not permission to drop editable interaction.

Pre-ledger uncommitted review material is preserved in
`.lf/tmp/loo-709-before-input-ledger.patch`; the earlier review page remains at
`39f57361dd:scratch/concept-review.html`. The current page reflects this review.

## Check result

Gate plan — 2026-11-03, HEAD `74880b72e3`, compared with `origin/main`:

- Transport/lifetime and input/activity store changes reach `transport_tests`,
  `transport::`, `store::sqlite::{native_input,steers,processes,program_status}`,
  and `store::migrations` (including the released-frontier upgrade fixture).
- Native hook, resume and bulk-command removal reach `config::`, `context_block`,
  `harness::context`, `lf::commands::{context,session,work_route,rollover}`,
  `lf::tests`, `binary(lf)`, `session_cli_tests`, `agent_startup_tests`,
  `session_launch_contract`, documented-command checks and `world_recipes` watch
  cases. Retained transport also reaches the interactive Graph and nested-launch
  acceptance boundaries named in the test README.
- Use the changed-aware runner with this Rust filter and `--reuse-passing`;
  its architecture, website and headless Swift suites cover the changed docs and
  deleted Desktop startup caller. It runs fmt and Clippy after building/tests.
- Run migration immutability first, then the disposable installation harness for
  the migration and nested-Graph boundary. Linux-only execution stays with CI.
  Native interaction/return measurements are already captured in the demo; this
  pass does not repeat them or claim installed acceptance.

Gate result — **passed**, 2026-11-03, with inherited `LF_*` unset and stdin closed.
Rust checks used stable 1.99.1. The initial gate was deliberately interrupted
during CLI compilation after discovering the retry defect; it reached no test
timeout. The completed run and focused repair below retain their separate results.

The policy previously forgot a failed viewer write when it advanced its activity
revision. A later successful store read could therefore see stale viewed status
forever. It now retries the latest viewed state until persistence succeeds.
Repository lookup failures also leave the configured grace unknown for retry,
instead of permanently choosing the global default. The new viewer-write
regression failed before this fix and passed after recovery with no new viewer
event. The output/quiet regression now also forces a Process read error and proves
recovery without new activity.

Two existing fixtures needed correction: rollover supplied unfenced terminal
status instead of owned provider work, and the policy clock fixture inserted
`trace_id='fixture'`, which cannot decode as a Process. The latter caused its
eight-second candidate receive to time out once read failures were correctly
retried. It now uses the existing valid Process fixture; rollover records activity
through a current attachment. No deadline was increased.

- `uv run python scripts/check_migrations.py` — passed; only `native_terminal_input.sql` is unmerged.
- `uv run python scripts/test.py --reuse-passing --rust-filter <planned owners above>` — architecture passed; Rust ran 438 tests (436 passed, the two diagnosed fixtures failed); Desktop app build and headless suite passed (663 tests reported, 44 optional/display or measurement tests skipped), and Swift boundary checks passed. Exact filter/argv and output: `.lf/tmp/gate/loo-709-gate.log`.
- `cargo nextest run -p loopflow --lib --no-fail-fast --build-jobs 4 --test-threads 4 -E 'test(transport::lifetime) | test(lf::commands::rollover::)'` under `scripts/test_network.py` — all 10 passed after fixture repairs; unchanged affected suites retain the completed run's evidence. Log: `.lf/tmp/gate/loo-709-repair.log`.
- `LF_BIN=<absolute candidate> uv run python dev.py test` from `website/` — 72 passed. The first attempt selected an older installed `lf` that could not emit the command catalog; pinning the built candidate resolved it. `scripts/testing.md` now records this prerequisite. Log: `.lf/tmp/gate/loo-709-website.log`.
- `uv run python scripts/test_task_installation.py --test declared_agent_can_start_another_tasks_graph` — all three isolated Linux proofs passed: planning migration preservation, prior Task delivery preservation, and nested Task Graph launch. Log: `.lf/tmp/gate/loo-709-installation.log`.
- `uv run python scripts/test_task_installation.py --repository-upgrade` — populated 0.14.3 promotion preserved Task/Session identity, history, edits, move and retry. Log: `.lf/tmp/gate/loo-709-upgrade.log`.
- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` — passed on macOS; the same all-targets Clippy command passed in a disposable Linux container using the installation harness's migration order/cache. Logs: `.lf/tmp/gate/loo-709-clippy-{macos,linux}.log`.

The macOS test linker still reports its existing large-unwind-section warning;
both Clippy runs are clean. The repaired Rust fixture inputs alone were rerun;
the final notes are prose-only. All created containers and check processes ended.
The prior configured [demo](loo-709-idle-demo.md) remains the native interaction,
startup and twenty-return evidence. Proceed through the authored delivery steps;
merged installed verification remains outstanding. Conservative native input
retention remains explicit, and Jack Heart's accepted interaction requirements
are unchanged.
