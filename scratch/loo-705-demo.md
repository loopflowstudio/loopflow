# LOO-705: installed lf handoff demo

2026-11-02, headless demo, decision pass 1. Reviewed build
`c788a6ea32e93f180f827441eba103ad53551886` against the
[current design](loo-705.md) and Jack Heart's instruction that viewers run
the owner's successor command, never replay their own argv.

## Decision: advance

The agent makes this decision under the explicit headless delegation to stand
in for Jack Heart. Jack did not attend this demonstration or give new approval.
The rationale is his requested experience: resume reaches the saved conversation
without manually killing its old PID, and an existing Graph uses the new lf for
its next step. Both now have execution evidence. The accepted design's known
limits remain; no new implementation change is proposed.

The preceding pass intended to unify removed, replaced and superseded owners,
preserve an answering owner's active turn, and send the complete successor
command. Its recorded PTY and takeover tests support those behaviors. It left
the configured Mac resume and a Graph crossing builds unproved. This pass adds
those observations; the lack of an earlier demo record was an evidence gap,
not evidence that the preceding implementation made no progress.

## Configured Mac Session

Ran this branch's `target/debug/lf resume LOO-618` in a pseudo-terminal, with
inherited `LF_*`, `LOOPFLOW_*` and `CMUX_*` authority removed. The main Home,
installed entry and configured Claude provider were used. No new prompt was
submitted. The Task status before the demo identified detached Session
`9f2a7eba105641dcb95656ba56cf78fd`.

Observed terminal output, in order:

```text
Stopped the claude client that had this conversation; continuing here.
This Session's lf was superseded; reopening it as `lf resume 9f2a7eba105641dcb95656ba56cf78fd`
[Claude displayed the existing LOO-618 conversation and an empty input prompt]
Detached from “LOO-618 lf resume always gets you back: default takeover, --fork, session new, t”; it keeps running.
```

The displayed history included LOO-618's previous release/verification findings
and its done status. Those historical statements were screen content, not new
findings or instructions. The terminal followed automatically after the notice.
The branch first reopened the provider and then yielded to the installed build;
this observation does not claim a single provider launch.

Read-only store and kernel inspection established:

- Before: agent Process `34ca1594-0aa0-4261-9ec8-0929602cc554` named Claude
  `--resume e370fcaf-eb69-41b6-8eef-ee55bd0798b6`.
- After: agent Process `c46f6a19-f02a-4aa2-9fbd-e0bcccdec9b4` named the same
  `--resume e370fcaf-eb69-41b6-8eef-ee55bd0798b6`.
- The new owner `e4a38c7b-04e2-455f-82e0-f84ec20fa676`, PID 65816, ran
  `/Users/jack/.lf/bin/lf-5854cc113ff596c596479426a567d305f27d2f48f5332fd57ad94aee1f8d4f44`.
  Both its recorded command and macOS `proc_pidpath` named that executable;
  `--version` reported `lf 0.15.1`.
- The new provider, PID 66340, was configured Claude 2.1.316. The saved Session
  identity and provider thread survived the handoff.

Cleanup: Ctrl-] detached the viewer and the foreground command returned. To
leave no demo-created background process, SIGTERM was sent only to the newly
created owner after checking its recorded ID, PID and start time. Both that
owner and its provider exited. The saved conversation was retained; no Session
close, Task completion or Graph restart was requested. Cleanup is not approval.

The Task's original Oct 28 owner was not re-established as the live owner before
this run. Therefore this proves configured conversation recovery and a branch
owner following the installed entry, not the exact original 0.15.0 socket
failure or an older-to-newer release installation on the main Machine.

## Graph across builds

`uv run python scratch/loo-705-graph-demo.py` — passed: one released runner,
two successful steps, first on released lf and second on the branch build.

The [reproducible driver](loo-705-graph-demo.py) creates a private Home and a
disposable repository with a local bare remote. A released 0.15.1 runner executes
two real `sync --plan` steps. After the first finishes, the private `LF_BIN`
entry switches to the branch executable. This exercises the supported private
entry path; it does not promote a Machine installation or simulate provider load.

[Recorded results](loo-705-graph-demo.json):

| Execution | Build | Result |
| --- | --- | --- |
| Runner `d2e4c3f0-605e-4d83-aa41-81b64845caa5` | released 0.15.1 | succeeded |
| Step `fbad9a8b-c58f-437f-b546-40f643383d52` | released 0.15.1 | succeeded, exit 0 |
| Step `5ae5a17b-652e-4c0f-a96e-7af938eb5109` | 0.15.1+c788a6ea3 | succeeded, exit 0 |

The same Graph retained both step receipts and exited normally. No provider,
Task, agent delegation or background server was started by this proof.

The [first driver result](loo-705-graph-demo-initial.json) also ran both steps
successfully, but its assertion incorrectly expected canonical executable paths
in recorded argv. A direct symlink launch records the entry spelling. The driver
now switches between wrappers that explicitly invoke each executable, making
the recorded path comparison valid. This was an evidence-driver correction,
not a product defect or a discarded product failure.

## Limits and next action

No new human feedback, agreed design change, or unresolved product question arose.
The recorded known limits still apply: pre-policy answering owners can require a
second resume; below-reader-floor owners cannot judge their own idleness; a Graph
whose store advances past its reader floor is not recovered by this mechanism.
This pass did not install a release, run bulk rollover, manufacture a schema or
wire incompatibility, or exercise native Desktop panes. The installed rollover
dry run was diagnostic only and is not evidence for this branch's sweep behavior.

Recommended next action: advance to the caller's next Graph step. Gate retains
its affected-suite responsibilities. The reason to advance is the newly observed
configured handoff with identical provider thread and the actual released Graph
runner executing its next step on the newer build, alongside the earlier focused
test evidence. No concrete repairable failure was demonstrated.
