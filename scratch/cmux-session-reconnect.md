# cmux session reconnect

Jack Heart reported `lf session connect aaa3bce933ca4809bddf84233dc51dbc`
failing with `Previous AgentProcess has no observed exit`, and a perceived hang
from `lf incident -i -c` on 2026-10-13.

The installed v0.13.13 executable has digest `aede68c4e815805766c605487c5fda94754f9f2f8566a3403782787b10b718b2`
and source revision `6935bfbadbc2cecca1321dda4bd6668b3d42a062`.
It predates `def8881d82` (#1542), which lets native reconnect observe a dead
interactive provider and retire its record. The installed implementation returns
false for interactive providers without checking death. `lf install` reports
v0.13.13 is still the latest published release; it changed nothing.

The original provider record `2583bbb6-35c8-454e-b70c-a8acb2addbc4` retained
PID 13910, birth 1791942372, `spawn_requested`, and no completion. Both that
PID and its launcher PID 9643 were absent. No native thread was recorded.
Failed reconnect inputs `d040c06a07f443fb891cdd3f00b5e5c5` and
`241b673da158402a9542ca5c0ab73e7d` remain in Session history.

After a dry run found no live orphan provider groups, `lf monitor prune --json`
completed with zero errors and zero terminated provider groups. The original
provider now has a completion time and `spawn_state=exited`, no invented outcome,
and no attached launcher. Session listing exposes an available Open here action.
This is a workaround for the installed release, not proof of native reconnect.

cmux inspection found two live tabs in Infrastructure Work: surface:1000040117
displays this unbreak investigation, descended from incident Process
`27a69302-b9d4-4507-b773-a554c269e8d3`; surface:1000040118 displays the separate
wave-session conversation. Both rendered ongoing work. The incident launched its
provider in the same second as invocation. No current hang was reproduced, and
the cause of the earlier perceived hang remains unresolved.

The Task's cmux surface:1000040116 already runs a newer conversation,
`session_fafd7eb4e1af4ca28143fc23b1d98577`. It was left untouched. A later reconnect
did pass the original authority error, but timed out before becoming resumable.
Jack reported that stall, then directed attention to bug fixes and causal analysis
because another usable conversation was already open.

## Why startup stalls

The retry's provider record `da55629a-7aa9-4680-bb39-b960e9a67f25` held PID
97527. OS inspection found Bash running, not Codex. Its parent invocation
`53a75273-99da-4688-bb4b-c5e74142ccf8` recorded the exact 30-second readiness
timeout for input `82927d1a91da487482525cba9eee29c6`. The provider survived its
launcher briefly as an orphan before disappearing; its exit was again unrecorded.

1. **Why was the terminal blank?** The installed cmux Codex shim invokes
   `cmux-codex-wrapper` before execing Codex. Its provider-selection scanner alone
   took 26.617 seconds when replayed with the failed launch's exact arguments.
   This check executed only the extracted scanner, without an agent, credentials,
   model request, or terminal mutation.
2. **Why does that scan take so long?** `codex_launch_has_subrouter_provider`
   runs Bash's `${assignment//[[:space:]]/}` over every `-c` value. The launch
   includes a 3,087-character hooks value and an 8,027-character developer
   instruction value. Neither is a provider selection. Removing whitespace from
   the instructions dominates the scan; the original scanner exceeded an
   eight-second bound, then completed in the separately timed 26.617-second run.
3. **Why does a startup delay strand the Session?** Loopflow's foreground opener
   allows 30 seconds for native history and liveness. Wrapper parsing consumes
   nearly that entire budget before provider startup. Timeout drops the child
   launcher with `kill_on_drop`, so its ordinary exit recorder can be skipped.
   The recorded timeout and orphan observation support this sequence; the exact
   original Oct 13 18:46 launch was not sampled live.
4. **Why does the next connect refuse?** v0.13.13's interactive-provider close
   callback returns false even with positive OS death evidence. The merged #1542
   change fixes this; the latest published release does not contain it. The local
   prune workaround records death without inventing a provider outcome.

## Fix and proof

`cmux-config-scan.patch` changes the wrapper to normalize the config key first
and process a value only for `model_provider`. It keeps later-value precedence,
quoting, whitespace handling and the `--` boundary. The same recorded arguments
complete in 0.005 seconds. Eight selection cases pass against both implementations.
The complete patched wrapper reached an isolated stand-in provider in 0.210
seconds using the original arguments, a temporary HOME, and disabled external
cmux effects. No actual provider or account was used by that proof.

The patch is prepared, not installed or submitted upstream. It belongs to cmux;
editing its signed application bundle is not necessary for Jack's revised focus
on durable bug fixes. Loopflow's existing #1542 fix needs release delivery, not
another duplicate implementation or a longer timeout. The stale-exit guard did
not cause the initial startup delay; removing it alone cannot fix the wrapper.

The review found one important limit: native history remained absent
from other running Codex Sessions too. The wrapper timing explains the observed
CPU stall but does not prove that faster startup alone restores native history.
Configured Codex hook delivery and a full native reconnect remain acceptance
work. A private-Machine native probe with the patched wrapper reached the actual
Codex 0.162.1 loading screen immediately, then Codex panicked in crossterm with
`reader source not set` in the tool-provided PTY. That probe does not validate
hook delivery or a real cmux terminal. It changed no installed application files.
No claim of end-to-end recovery is supported yet.

Reproduce the patch checks without any account, agent or cmux side effects:

```sh
uv run python scratch/check-cmux-config-scan.py \
  /Applications/cmux.app/Contents/Resources/bin/cmux-codex-wrapper \
  scratch/cmux-config-scan.patch
```

Check: `lf monitor prune --json` passed; original reconnect advanced to a new
startup timeout; isolated original/patched scanner replay and eight selection
cases passed; complete patched-wrapper stand-in launch passed in 0.210s;
signed-in native reconnect remains unverified.

## Prevention without modifying cmux

Jack asked for commands resilient to other terminal wrappers too. A private
startup probe sent 1,100,000 bytes of developer instructions to the actual
Codex app-server through the unmodified installed cmux wrapper, with a 94-byte
command line. Initialization completed in 0.061s and thread creation in 0.070s.
The probe used an empty temporary provider home and an ephemeral thread; it did
not send a model turn or use an account. It establishes that the existing
structured transport removes large Loopflow payloads from wrapper parsing.

`infra-bugs.md` describes the remaining native-session integration, lifecycle
ownership and acceptance boundaries. That initial probe established the transport direction only; implementation and
native-terminal evidence follow below. No installed wrapper was changed.

## Implemented recovery path (2026-10-13)

Codex native launches now reuse the existing app-server and fenced connection
instead of putting prompts, developer instructions and hooks in argv. The native
terminal creates/resumes the thread and owns first-turn approvals. Failed startup
keeps the original captured input for reconnect. Claude and OpenCode are unchanged.

Actual Codex 0.162.1 through the unmodified cmux wrapper returned the initial
requested answer in 5.96s and a follow-up after reconnect in 8.57s, both exit 0.
The isolated Machine contains one Session, two completed turns/outputs, successful
context hooks and zero live providers. Provider argv was 125 bytes. Evidence is
under `/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-native-transport-smoke-4cjkwi1d`;
terminal logs and the Machine database are local, temporary diagnostics. The probe
used a real PTY and account but disabled cmux pane/title effects; it did not reopen
the originally reported pane or alter the installed lf release.

Counterexamples corrected during implementation: an empty Codex thread cannot be
resumed before its first turn; remote clients reject `--no-daemon` and `--add-dir`;
a passive resubscription must not attempt an attachment write. The revised fixture
models the first two. The actual-provider attachment-transfer regression also
exposed a pre-existing history-origin lookup that closed a stale client's socket
instead of rejecting only its input; that error now follows ordinary RPC rejection. The terminal driver's first reconnect attempt pasted a
follow-up without submitting it; separate text/Enter writes completed the check.

The final transport and checks are recorded in `infra-bugs.md`. Release delivery
remains; the earlier perceived incident-command hang still has no reproduced cause.
