# Pending Codex approval survives client handoff

2026-09-28 · LOO-298 · Bounded native protocol experiment requested by Jack Heart.

**Confirmed for the tested ordering:** installed Codex 0.157.1 replayed an
outstanding command approval when client B resumed client A's active thread.
After A disconnected, B answered that received request once. The command ran
once, its turn completed, and an active sibling thread survived and subsequently
completed. One candidate ran successfully in 1.086 seconds; no retry was needed.

The replay arrived **during B's resume, before A disconnected**. This establishes
A receives → B resumes → A disconnects → B answers. It does not establish the
alternative ordering where A disconnects before B connects.

## Inputs and isolation

Read finding 2 in `scratch/reviews/parallel-handoff-history-review.md` and the transport
patterns in `tests/e2e/codex_connect.py`; neither file was edited. Used the
installed executable's generated JSON schema for the approval response shape.
An initial lookup used the wrong schema subdirectory; the root schema files
resolved that inspection error. No experimental candidate failed.

- Executable: `/Users/jack/.codex/packages/standalone/releases/0.157.1-aarch64-apple-darwin/bin/codex`
- Version: `codex-cli 0.157.1`
- SHA-256: `27ceb5f9b957b43a519efe4eaa3816a0bffb0a531a2c89af18840c0a3c016a7d`
- Transport: actual app-server, Unix WebSocket, three independently initialized clients.
- Explicit environment only: system `PATH`, private `HOME`, private `CODEX_HOME`, `LANG=en_US.UTF-8`. No inherited LF authority or credentials.
- Provider: credential-free localhost synthetic Responses server, `requires_openai_auth=false`. No configured account or real model request.
- Policy: `approvalPolicy=on-request`, `sandbox=workspace-write`; shell snapshots, analytics and feedback disabled.
- Whole-candidate bound: 180 seconds, with owned-process cleanup in `finally`.

The synthetic response emitted an actual `exec_command` function call with
`sandbox_permissions=require_escalated`, an explicit justification, and
`printf 'approved-once\n' >> approval-count.txt` in the private working directory.
**Codex itself issued the approval request.** The fixture did not manufacture a
server approval message. The marker was absent both when A received the request
and immediately before B answered. Native startup discovered the repository's
AGENTS.md above the private working directory; the probe did not edit it.

## Observed protocol sequence

Selected thread: `01a0e9a1-6459-7fa1-ab2d-f55d4853bf54`.
Selected turn: `01a0e9a1-65af-7580-9ded-5503bca9a5ac`.
Sibling thread: `01a0e9a1-6555-7023-91f8-fd6339d20121`.

| Event | Monotonic timestamp | Evidence |
| --- | ---: | --- |
| A receives approval and withholds response | 1464425.014620083 | `item/commandExecution/requestApproval`, request ID `0`, item `call_approval` |
| B initializes after the original request | 1464425.034326291 | B could not have received the original broadcast |
| B sends `thread/resume` | 1464425.064455166 | Exact selected thread ID |
| B receives replay | 1464425.066970500 | Same request ID, thread, turn, item, command and parameters |
| A disconnect completes | 1464425.089771208 | Socket closed before B's answer |
| B answers | 1464425.089960500 | `{"id":0,"result":{"decision":"accept"}}` sent once |
| B receives resolution | 1464425.090257500 | `serverRequest/resolved`, request ID `0` |
| Command completes | 1464425.119545583 | `call_approval`, exit code `0` |
| Selected turn completes | 1464425.128257208 | Status `completed` |
| Sibling is read | 1464425.150202625 | Thread `active`, turn `inProgress`, no completion notification |
| Sibling completes after fixture releases its held response | 1464425.182382500 | Status `completed` |

The private marker contains exactly one `approved-once` line. The sibling's
synthetic upstream response was deliberately held throughout the approval and
selected-turn completion. There were three synthetic upstream requests overall:
selected tool call, selected completion, and held sibling completion.

**Implementation consequence:** the receiving client must process server requests
while awaiting the `thread/resume` response, retain the received approval ID, and
handle `serverRequest/resolved`. The tested native path supplies the pending
request; no separate recovery API or guessed ID was necessary. Loopflow's driver
fence remains a separate implementation obligation.

## Reproduction and retained evidence

From this checkout, select a fresh output directory:

```sh
uv run .lf/tmp/approval-replay-probe/probe.py \
  --output .lf/tmp/approval-replay-probe/candidate-2
```

The executed command used `candidate-1`. Script, generated schema and all evidence
are confined to `.lf/tmp/approval-replay-probe/`. Candidate evidence:

- `candidate-1/results.json`: executable identity, environment, decoded approvals,
  resume/read results, completions, timing, and cleanup receipt.
- `candidate-1/client-A.json`, `client-B.json`, `client-S.json`: complete recorded
  sends and receives with monotonic timestamps.
- `candidate-1/upstream-requests.json`: synthetic Responses inputs.
- `candidate-1/engine.log`: empty; no engine diagnostic failure observed.
- `candidate-1/work/approval-count.txt`: command side-effect evidence.

Engine PID and process group were **70888**, with retained start identity
`Mon Sep 28 13:07:43 2026`. The engine was alive after both turns. Cleanup reread
and matched start identity and process group, sent SIGTERM only to that owned
group, and waited for exit **0**. No SIGKILL was needed. Client sockets and the
local server closed; the probe's Unix socket was removed. Private history and
receipts remain for inspection.

## Limits

This is actual native protocol and shell execution with synthetic model output.
It proves one command approval replay and one accepted execution across the
tested connection ordering. It does not establish Loopflow claim transfer,
rejection of retained old clients, UI/Desktop behavior, durable usage or outcome
capture, engine restart recovery, a period with no connected client, other
approval kinds, or duplicate-response semantics. A single marker is evidence of
one execution here, not a general distributed exactly-once guarantee.

No source/test edits, builds, Git/PR/PM mutations, lf runtime mutations, installed
Home/account changes, or delegates were used. The only authored handoff is this
note; all other created files belong to the private probe directory.
