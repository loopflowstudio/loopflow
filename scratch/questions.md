# Open questions

## Resolved: rebased onto origin/main

Rebased onto `601881cd waves: reshape roster into wave/project/task ontology`.
Twenty-one conflicts, all one shape: main rewrote prose this branch was
renaming. Resolution rule throughout — **main's prose, this branch's grammar**.

Main's `PmCommand` is a strict superset (`--project` filtering, `--pr` on
close, `pm sync --plan`, and the `pm task create/update/done/move` family), so
the enum merged intact; only the parent path changed from `lf op pm` to
`lf pm`. Goldens were regenerated from the resolved prompts rather than
hand-merged.

Two sweeps beyond conflict resolution, both because main *added* `lf op`
references to files this branch's rename commits had already passed over:

- `05c4619a` — main's reshape put `lf op pm show` in every builtin skill's
  orientation line, plus `docs/index.md` and a `pm.rs` error string.
- `f7c21caf` — `Wave.command` (Swift) appended `lf op commit --push`, a
  command this branch deletes. Every Concerto-launched session would have
  failed at the commit step. This predates the rebase; it was never a
  conflict, just a miss.

Deliberately left carrying `lf op`: `release/*` notes and `RELEASE_NOTES.md`
(shipped records), the recorded turn fixture in `WaveChatConnectionTests.swift`
(captured data), and `wave/*/MEMORY.md` (server-owned).

## Latent: `op: rebase --plan` in a flow silently does nothing

`execute_flow_ops` (`src/ops/flow.rs`) returns `Ok(())` for a `rebase --plan`
flow item without planning or printing anything — the flag exists so the *CLI*
can dry-run, and the flow path has nowhere to print a plan. A flow step that
reports success while doing nothing is a footgun.

No builtin flow uses it (the only `op:` payloads in-tree are `pr open`,
`pr land`, `pr land --create-pr`, and `release run patch`), so nothing is
broken today. Left as-is because turning the no-op into an error is a behavior
change, not a reduction. Assumption: a future pass makes `--plan` an
unsupported flow op rather than a silent success.

## Untested: the two new exec-door denials

`wave_exec_verdict` (`src/wave/server.rs`) grew `Deny` arms for `auth`,
`sync-skills`, and `task`. Only `auth` is covered by
`wave_exec_policy_rejects_dangerous_verbs`. Adding `sync-skills` and `task` to
that list is one line each; skipped here because this pass only removes code.
