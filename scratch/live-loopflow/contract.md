# LOO-295 live loopflow demo

Run the branch development executable with the existing ambient Codex login.
Use its isolated development Home; do not install, push, merge, or replace the
real Task's saved execution. This ordinary Flow tests the shared cursor's live
provider and terminal path; Task parity still needs separate evidence.

## Observable finish

An explicit human Advance starts a bounded writing Run. loop-decide directs a
second fresh writing Run through Iterate, carrying specific remaining work.
Once the writing is complete, the decision agent needs the human's observation
of the actual approval handoff before it can honestly finish the walkthrough.
It calls `lf flow blocked` with that precise missing evidence, opening an
unblock Ask. Human Complete returns the answer to that same decision Run.
The agent records it, reassesses, and Advances to one final human demo gate.
The human explicitly confirms the demonstration before the final Advance.

## Artifact acceptance

scratch/live-loopflow/walkthrough.md must contain:
- A concise explanation of Advance and Iterate as forward and backward Flow
  transitions, with Blocked as a request for evidence or judgment through Ask.
- Two distinct writing Run IDs and the carried direction used by the second.
- The human's actual observation of the initial approval-to-worker handoff,
  received through the Ask. Never invent this evidence or infer it from exit.

The first author pass intentionally covers only Advance. This is a staged
transport exercise, not evidence that an autonomous reviewer discovered a
real implementation defect. After the second pass, do not Iterate merely to
obtain human evidence: Ask the human whether the Ghostty approval handoff made
the next action clear, and what felt confusing if anything. The decision agent
may write that answer into the walkthrough after Ask Complete returns. If the
answer reveals a real defect, record the gap and stop for implementation rather
than claiming the demo passed.

Do not treat this exercise as whole-branch acceptance or Task completion.
Source review, hermetic tests, and a parked/ready Session are not live success.

## Evidence

Setup: development build succeeds. No managed accounts in its isolated Home;
`codex login status` confirms the ambient ChatGPT login and development
`lf auth status codex` resolves it. No provider credential values were read or
printed.

Invocation: `41729e12-84b5-4ee8-9ca9-4ae836a20b76`.
Opening boundary: `bd6c5338-d629-4908-8078-b92b1119ef7b`.
Ghostty window: `tab-group-abcd3dfe0`. The development executable's Session
list initially reported this exact Flow Session active, then waiting. It parked
before any author Run. The installed app reads a different Home and does not
display this development Session.

### Failed live launch and recovery — 2026-09-25

Observed:
- Ghostty displayed Codex's sign-in screen, followed by `Error: human Session
  Run did not become resumable within 30s`. The process exited.
- The saved boundary retained Run `run_6672e27de94e493799dd69efc516fa9e`.
  Its manifest had `launch: null`; the only recorded event was initial input.
  `lf top` for this development Home showed no live Loopflow call trees.
- Ghostty's application environment carried older `LF_HOME` and
  `LF_CONTROL_HOME` values. This is a possible contributor to the auth failure,
  not a proven cause: the shell's ambient and CODEX_HOME-cleared
  `codex login status` both reported a ChatGPT login.
- Retried the same exact Session with stale LF Home, control, Run, Session,
  account-lease, and installation variables cleared and LF_BIN pinned to the
  branch executable. New Ghostty window: `tab-group-abc1b9040`.
- Retry failed: `Flow Session ... has Run run_6672e27de94e493799dd69efc516fa9e
  but native history is unavailable`. The Session list still reports waiting.
- No human approval, writing pass, Iterate, Ask, or final gate occurred. This
  is a failed demonstration, not acceptance. The failed invocation is retained
  unchanged as a reproduction; no replacement Flow was used to hide it.

Next implementation pass:
1. Repair recovery of a human Flow boundary whose launch never published a
   native identity. Preserve published native identities; safely retry an
   unpublished failed launch through the existing launch lock and ownership
   checks. `flow_session::open` currently treats any bound Run as resumable.
2. Trace why the actual TUI launch asks for login despite successful shell
   auth status. Reproduce with the same launcher environment before choosing
   a credential-routing fix; do not copy secrets or install to bypass it.
3. Reopen this retained invocation and complete the live sequence with Jack.
   Hermetic tests remain useful but cannot satisfy this live proof.

### Retry repair

Jack requested another attempt. The exact saved Session still failed with
native history unavailable. `flow_session::open` now recovers an unpublished
Run under the existing Session launch lock: only when its manifest is readable,
no native session was published, and no owned provider client is alive, clear
the boundary's transient Run binding and readiness. Retain the Run artifacts,
boundary identity, cursor, and approval. Published native identities remain
resumable; an active startup is left alone. A completed boundary rejects recovery.

Two focused tests pass, nextest `79523ef0-c0c3-4811-a016-142f56e5fc92`:
failed-launch/live-client/published-native recovery and existing nested exact
approval. Formatting and all-target Clippy pass. These are local regression
checks; successful live reopening remains pending.

### Credential discriminator

The repaired opener retried this boundary as Run
`run_ddcb801aca45475a93bdc3ac9c5ac345`. With inherited Home/account context
cleared, it still showed sign-in. A bare native `codex login status` executed
through Ghostty succeeded, ruling out a universally broken Ghostty login.

A temporary executable shim under `/tmp/lf-demo-auth-probe/` recorded only
variable presence, ran native login status, then executed the same real Codex
binary with only `CODEX_ACCESS_TOKEN` removed. The two probes were:
- Without the injected variable: `Logged in using ChatGPT`.
- With the injected variable: `Error checking login status: agent identity JWT
  payload is not valid JSON`.

The actual review with that variable removed published a native Session and
became ready as `run_ccf77d7dfc964b38b6b7440cf665dfb9`, on the same invocation
and boundary. This diagnostic shim is not normal-path acceptance. No secret
values were read, printed, or copied. `launch: null` is normal for TUI manifests;
native receipt and owned client are the publication evidence.

The stored Codex OAuth mapping now returns no process environment variable;
native Codex owns its ChatGPT login. This avoids misrepresenting a ChatGPT token
as the CLI's agent-identity credential. Existing API-key and managed native-Home
routes remain intact. The separate forwarded-account lease path also exports
CODEX_ACCESS_TOKEN and needs its own supported-auth audit; it is not exercised
by this local, no-managed-account demo.

The three focused credential/real-launch-adapter regressions pass (nextest
`e7935a81-c1cd-4525-bde2-bec5262548ca`), as do formatting, all-target Clippy,
and the rebuilt development CLI. The temporary `codex` shim was renamed to
`codex.used`, removing it from executable resolution while retaining diagnostic
evidence. Subsequent providers use the ordinary native executable and repaired
stored-token mapping. The existing authenticated review remains ready; human
Advance and subsequent worker startup are still pending.
