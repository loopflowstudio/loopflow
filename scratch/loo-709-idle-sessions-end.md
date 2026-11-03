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

## Current implementation and input authority

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
no deletion targets remain in this cut.

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

## Remaining acceptance and delivery

1. Gate the input-ledger and activity regressions alongside existing silent-turn,
   watch, Graph and headless cases. Include rejected prompt/startup boundaries and
   the queued idle-ending/new-viewer race; focused proof is recorded below.
2. Demo through the configured/installed path and inspect slow/failed startup
   without replay. The real native probe below uses a local model fixture; it
   does not establish installed or configured-account behavior.
3. Complete all twenty returns at the bottom without new turns. Prior evidence
   ended all twenty owners but resumed only three; do not call that acceptance.
4. Gate owns the affected suites and Clippy, plus the schema upgrade boundary.
   Affected Rust owners include `store::sqlite::native_input`, `transport::lifetime`,
   `store::sqlite::{steers,processes,program_status}`, `context_block`,
   `harness::context`, `lf::commands::{context,session,work_route,rollover}`,
   CLI parsing/explanation (`binary(lf)`), `transport_tests`,
   `integration::session_cli_tests`, launch-contract checks and `world_recipes`
   watch cases. Narrow Swift deletion still needs headless compilation of
   RegistryQuery and LoopflowApp consumers; documentation checks also apply.
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
Pre-ledger uncommitted review material is preserved in
`.lf/tmp/loo-709-before-input-ledger.patch` and `scratch/concept-review.html`.

## Check result

With inherited `LF_*` unset and stdin closed, `cargo test -p loopflow --lib --test transport_tests -- store::sqlite::native_input native_input_acknowledgements silent_native_turns_follow_the_provider native_drafts_remain_editable silent_native_turns_survive native_background_watches_survive transport::lifetime`: 12 passed. `cargo fmt --check`, diff and context limits passed. Earlier unchanged migration inventory/upgrade and headless Desktop proof (`scripts/test_desktop.sh --filter RegistryQueryTests -Xswiftc -gnone`, app/library compiled, 17 tests) remains applicable. Gate owns broader affected suites and Clippy; demo owns configured/installed acceptance and all twenty returns. No publication or landing in this step.
