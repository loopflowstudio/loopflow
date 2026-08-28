# Execution

Execution turns one Skill into one provider result and one Home-local Run
record. It has no durable planning prerequisite.

```bash
lf implement
```

That ordinary path needs no Wave, Project, Task, daemon, or planning database.

## Request flow

```text
argv
  |
  v
CLI dispatch
  |
  v
Skill discovery --> context sources --> assembled prompt
                                         |
                                         v
provider route --> credential lease --> harness subprocess
                                         |
                      +------------------+------------------+
                      v                  v                  v
                 conversation        usage events       raw output
                      +------------------+------------------+
                                         |
                                         v
                                   Run record writer
```

| Stage | Input | Output | Source owner |
| --- | --- | --- | --- |
| Dispatch | argv, environment, cwd | selected command and launch flags | [`lf/mod.rs`](../../rust/loopflow/src/lf/mod.rs) |
| Discover | Skill or Flow name | one concrete source | [`lf/discovery.rs`](../../rust/loopflow/src/lf/discovery.rs) |
| Prepare | agent docs, Skill, directions, preassembled context, explicit docs/diff/message | system and task prompts | [`engine/prompt.rs`](../../rust/loopflow/src/engine/prompt.rs) |
| Route | profile, account health, model request | harness, account, model, credential | [`provider_account.rs`](../../rust/loopflow/src/provider_account.rs) and [`provider_account/lease.rs`](../../rust/loopflow/src/provider_account/lease.rs) |
| Spawn | prompt, route, environment | provider process and normalized stream | [`harness/`](../../rust/loopflow/src/harness/) |
| Record | launch facts and normalized events | immutable record plus disposable read model | [`run_record.rs`](../../rust/loopflow/src/run_record.rs) |

## Add optional Work attribution

```bash
LF_DB_PATH=/unreadable lf --task LOO-265 implement
```

The Task selector can enrich the prompt when planning storage is available. It
is still recorded as attribution when the store cannot be read, and the
provider still launches. Attribution describes the launch; it does not reserve
Work or authorize a mutation.

## Find and assemble the Skill

Skill discovery searches repository overrides, builtins, and installed Skill
directories. A Flow node resolves through the same mechanism. One source wins;
there is no runtime merge between two implementations of the same Skill.

The prompt engine assembles only declared or explicit context:

- system and surface instructions;
- `AGENTS.md` or `CLAUDE.md` and the Loopflow operating contract;
- the Skill and requested directions;
- Wave goal, memory, and selected Work context when available;
- explicit documents, changed-file bodies, diff, clipboard, and user message.

Execution does not resolve planning state. The CLI or controller resolves
Wave/Work context first—through [`work/`](../../rust/loopflow/src/work/) when
applicable—and passes plain prompt inputs to the engine. A direct Skill run
therefore remains valid when planning storage is absent or unreadable.

Prompt attribution can report where tokens came from. It is observation, not a
lease over the repository or planning state.

## Select a provider route

The current Home's access profile supplies an ordered set of provider accounts.
Routing selects one harness, account, model, and credential before manifest
publication. The stable non-secret account ID is recorded with the launch
request; credential bytes and leases are not. A credential retry may move to
another account while staying inside the same Run.

Credentials remain in provider-native homes, encrypted storage, Doppler, or an
explicit foreground SSH lease. A detached process uses credentials installed
on its own Home. The Run record can name the account but never contains its
credential.

## Publish before spawn

`CaptureHandle` mints a `RunId` and publishes the manifest immediately before
the provider subprocess starts:

```text
$LF_HOME/runs/<first-two-uuid-chars>/<run-id>/
  manifest.json       required; immutable
  context.json        optional; immutable exact provider strings and attributed assets
  events.jsonl        optional; append-only lifecycle, conversation, provider, and usage evidence
  terminal.json       optional; immutable terminal proof
```

Manifest creation is the only required new persistence step:

1. Build `RunSpec` from facts available at launch.
2. When prepared context is available, write `context.json` with the exact
   system/task strings, ordered attribution, hashes, byte ranges, token counts,
   and inclusion decisions.
3. Write `manifest.json` with the context path, hash, and byte count inside the
   same private staging directory.
4. Sync the files, rename the directory atomically, then sync its parent.
5. Export `LF_RUN_ID` and `LF_RUN_DIR` to the child.
6. Export `LF_PARENT_RUN_ID` only for a verified local parent.
7. Spawn the provider.

The manifest records launch identity and attribution: Run and optional parent,
time, harness/model/surface, cwd/repository/worktree, Skill, subjects, optional
context reference, runtime artifact, host, and boot identity when available. It records no liveness,
credential, mutable Work state, or signal target.

Core types:

| Type | Contract |
| --- | --- |
| `RunId` | Opaque launch-evidence identity |
| `RunSpec` | Launch facts available before publication |
| `RunManifest` | Immutable, serialized launch record |
| `CaptureHandle` | Bounded best-effort event writer plus synchronous settlement |
| `TerminalReceipt` | First terminal outcome; exclusive-create wins |
| `RunSnapshot` | Disposable projection rebuilt by scanning records |
| `RunUsage` | Provider-authored usage reduced by stream |

## Observe without gating

Harness adapters normalize provider output into conversation, tool lifecycle,
session, retry, and usage events. The recorder uses a bounded in-process queue.
A full queue, broken file, or malformed optional event warns once and lets the
provider continue. JSONL does not sync per event.

Usage remains provider evidence:

- counters stay cumulative when the provider reports cumulative counters;
- each provider Turn has a distinct `usage_stream_id`;
- each retry has a distinct `attempt_key`;
- omissions and counter resets remain visible;
- `final_receipt` is provider-authored;
- Run settlement never invents provider finality.

A reader reduces each cumulative stream once. It never sums every checkpoint
as independent consumption.

## Settle once

When the harness returns, settlement creates `terminal.json` with `completed`,
`failed`, or `interrupted`. Creation is exclusive and synced. A second writer
with a conflicting outcome loses without changing the first receipt.

The event writer gets a bounded final drain. Telemetry loss cannot hold
settlement open or turn a successful provider result into failure.

If the launcher disappears before settlement, the record remains
unterminated. That means only “no terminal proof was recorded.” It is not proof
that a process is still alive.

## Read the evidence

```bash
lf runs --task LOO-265 --json
lf runs run_ab12 --events
lf replay run_ab12
lf usage --days 30 --json
```

`scan_runs_since` reduces record files into `RunSnapshot`. `lf runs`, `lf
usage`, Work activity, status views, and the Mac app consume that projection.
There is no authoritative Run index to repair.

Replay resolves one full Run ID or unambiguous prefix, verifies that its
manifest contains a headless launch request and that the named provider account
is available on this Home, then creates a normal child Run through the same
harness. Current prompt files and planning Work state do not reconstruct the
request; current credentials and repository contents remain live launch inputs.
The source record remains immutable.

Reads are local to the current Home:

```bash
lf runs                         # this Home
lf ssh build-home runs          # build-home
lf ssh build-home usage --json  # usage recorded there
```

## Boundary contracts

- One mediated harness launch publishes one manifest before spawn.
- One launch creates at most one terminal receipt.
- Append-only telemetry is useful evidence and a nonfatal side channel.
- One retry is another attempt inside the same Run.
- Run parentage and subject attribution describe causality, not authority.
- No `owner.json` means no durable cross-process signal authority.
- The direct spawner may cancel the child handle it owns; another process may
  not infer that capability from PID, Work, tmux, or Run-record evidence.
- Planning enrichment can add context. It cannot reserve Work or become a
  launch gate.

## Next

[Planning →](planning.md) composes runs and preserves purpose across them.
[Data and persistence →](data.md) explains how Run-record evidence relates to the
other stores.
