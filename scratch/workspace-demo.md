# LOO-498 workspace demo — November 5, 2026

Jack Heart requested `$demo` in the primary Task conversation. The Task is at
feature → demo; no merge or Auto-Ship approval follows. Current contract:
[docs/workspaces.md](../docs/workspaces.md). Design: [rex-workspace.md](rex-workspace.md).
Review page: [demo.html](demo.html).

## Current outcome: configured demo blocked

The branch executable refuses the shared database because `rex_workspaces` is
unmerged. This is the current migration policy, distinct from the November 4
catalog-order failure below. Promotion preflight reports shared frontier
`task_session_promotion`, pending `rex_workspaces`, and rejects promotion.
No live migration or installation was performed in this review.

The copied executable is `0.15.1+5c0d6419c` (source
`5c0d6419c96fd98386bf058cb22838249c8855c5`). Only scratch changed between that
revision and checkout HEAD `1606cf27df`, so the demo binary matches product code.

A consistent snapshot of Jack's real store opened successfully under a private
LF_HOME. `lf open --host rex --task LOO-498 --copy 'LOO-498 November 5 demo'
--new-shells --no-focus --json` created a native destination, with zero ordinary
shells: the selected saved layout contained one Conversation pane. The native
receipt was `opening`; Conversation readiness was **failed**, reporting the
unavailable execution-owner socket. No socket was manually wired, no provider was
started, and no input was replayed. Snapshot metadata does not supply a live owner.
The receipt's `rex_hold: held` is inventory evidence only; cleanup integration
with LOO-717 is still unfinished.

Rex accessibility showed the new sidebar entry. Selecting it was interrupted by
an app change. A fresh read then showed it absent, and a successful native
`session.list` confirmed the entire demo session absent. The cause of removal is
unknown. No input was sent to the newly selected unrelated conversation. The demo
was not recreated. Reuse, actual Split, before/after screenshots and manual-resume
interaction were **not** verified in this review. No acceptance or new product
direction was received from Jack.

## Durable snapshot and runtime

- Durable root: `/Users/jack/.local/share/loopflow/demos/loo498-20261105/data`
- Private LF_HOME: `/Users/jack/.local/share/loopflow/demos/loo498-20261105/data/lf`
- Runtime root: `/Users/jack/.local/share/loopflow/demos/loo498-20261105/runtime`
- Absolute LF_BIN: `/Users/jack/.local/share/loopflow/demos/loo498-20261105/runtime/lf`
- Source DB: `/Users/jack/.lf/loopflow.db`; SQLite backup, not a new synthetic plan.
- Evidence: runtime `status.json`, `open.json`, their stderr files; root `demo.json`.
- LF Session shown: `310fb7b6015843e498316cd921a348b9` (existing primary,
  copied metadata only; no new execution owner or provider history created).
- Rex session: `session:01a1acb2-ba66-73d0-b507-1894e5d543cf`
- Rex window: `window:01a1acb2-bad4-70a7-b93f-a234023c395c`
- Rex block: `block:01a1acb2-bad3-7363-8026-9fec374687da`
- Native block owner: `client:01a18f6e-df25-75c4-8793-1c0ec64bd729`;
  host `6cd4509a-df69-4a66-88f3-53eaa4d8523e`.
- Configured Rex endpoint: `unix:///Users/jack/Library/Application%20Support/rex/server.sock`.

Keep both roots intact. No human draft/request settlement answer has been received;
closing the native session is not that answer. The agent sent no terminal input and
started no provider. After review input is settled and a fresh inventory confirms
no demo owners, runtime-only teardown is:

```bash
rm -r /Users/jack/.local/share/loopflow/demos/loo498-20261105/runtime
```

Never remove the durable root. The current native session is already absent; do
not target unrelated Rex owners. Private recorded Work can be reopened without
launching a provider (these paths remain fixed while the runtime is retained):

```bash
env LF_HOME=/Users/jack/.local/share/loopflow/demos/loo498-20261105/data/lf \
  LF_BIN=/Users/jack/.local/share/loopflow/demos/loo498-20261105/runtime/lf \
  /Users/jack/.local/share/loopflow/demos/loo498-20261105/runtime/lf task status LOO-498 --json
```

This snapshot contains database history, not a copied provider home. It cannot
resume the live owner or reproduce unfinished provider input. Resume the original
conversation through the installed CLI and original Machine when needed.

The local review page was written, but browser automation rejected its `file:` URL
(protocol policy permits only HTTP/HTTPS). No alternate server or browser workaround
was attempted; the conversation supplies the local artifact link.

## Next proof and review question

Recommend repairing the supported branch-demo path so real Work and its live
owner are available together; do not merge or migrate the shared store merely to
obtain a demo. Then demonstrate automatic opening and manual resume, Rex's actual
Split at the Task checkout, `cd` followed by Split, and repeat opening preserving
native edits. Do not claim those checks from labels or isolated fixture results.

Ask Jack what appeared before the new Rex session disappeared, and whether any
input there needs preserving. The runbook keeps that observation and draft
settlement question alongside the failed result. No design change was agreed in
this review. The following dated evidence remains historical.

---

# LOO-498 workspace demo — November 3–4, 2026

Jack Heart invoked `$demo` in the primary conversation, outside the then-stopped
feature Graph. The user contract is [docs/workspaces.md](../docs/workspaces.md),
the current design is [rex-workspace.md](rex-workspace.md), and the interactive
runbook is [demo.html](demo.html). Full dated observations and installation
preflight details remain at `7b886d04cc:scratch/workspace-demo.md`.

## Review outcome

Jack requested real Tasks, Waves and conversations for acceptance. Synthetic Work
and manually connected observer sockets remain supporting evidence. No native
Split result, successful app-selection screenshot or observed-behavior acceptance
was obtained. Computer use stopped when Jack changed the app; no further input was
sent to the new selection. No provider or Task Graph was launched for these demos.

After the real-data snapshot demo, Jack selected one-way LF templates: Rex owns
its layouts, shells and restoration. That accepted design supersedes the older
Rex away/closure/settlement model. Build and review publication through authored
demo are authorized; Auto-Ship and merge are not.

## Real-data snapshot — November 4

A consistent backup at `/private/tmp/loo498-real-demo-j8rn_nmv/lf/loopflow.db`
uses the branch build through `/private/tmp/loo498-real-demo-j8rn_nmv/lf-demo`.
The supported `LOOPFLOW_MIGRATION_ORDER` input supplied first-parent migration
order; a fresh target without compiler cache repaired missing dependency metadata.
Build and Task reads passed; this did not diagnose automatic catalog discovery.

Opening LOO-498 created session `session:01a1a7c1-f4fb-7ec1-bd1c-c7f87bcf9504`
and window `window:01a1a7c1-f577-7e6f-9fa3-22f9a30a9d7d`. Reopening preserved
the native layout and created zero extra shells. Task view showed the real brief;
the shell prompt showed the Task checkout, not proof of later native-split cwd.

The copied Home lacked the existing owner's socket, so the observer initially
failed. A symlink for that one socket and replacement of only the exited observer
block produced `block:01a1a7c4-3dec-7829-a19a-8d7c0465131a`, displaying live
conversation output without input takeover. This manual wiring is not configured
observer readiness. Rex accessibility showed the sidebar entry, but selection
was interrupted before an after screenshot or Split result.

## Installation and catalog evidence — November 4

Jack explicitly requested the dev stream after rejecting synthetic-only review.
`lf install --dev` installed CLI `0.15.1+ac8e088f5`, recorded dev, and migrated the
shared store to `wave_removal`. The explicit `~/.local/bin/lf` then passed exact
schema preflight and Task/Session reads; the scheduled installer remained registered.
The inspected package was GitHub run `41774946128`, revision
`ac8e088f5f4d18db1fdb5d7c4e7685c86ec9a004`.

Desktop was not updated by that installer. This conversation's inherited bundled
`LF_BIN` still selected old 0.15.1, which could not read the upgraded store. A second
install downloaded matching Desktop but refused replacement: PID 43082 lacked a
provable kernel executable path; mapped-image inspection pointed to an older app
in Trash. Twenty app-named helpers had unreadable executable paths. Rollover dry
run included other conversations and in-progress turns with unavailable sockets;
it was not applied, and no helpers were manually stopped.

Main's later #1815 (`dfd454cfc7`, merged into this checkout) changes installer
coordination. It does not erase the failed installation or prove a successful
current install. No further installation occurred during reconciliation.

The earlier configured branch open failed before materialization with a migration
history/schema mismatch. A fresh rebuild at `da41d2031` also failed preflight:
the branch catalog alphabetically ordered unmerged slugs, putting `rex_workspaces`
before `ship_stop`, despite the correct first-parent `origin/main` history.
Executable age alone did not explain it. Preview or a new release was never shown
to repair this; the unmerged branch is separate from the dev stream.

## Retained demonstration destinations

These are dated locators, not fresh host inventory:

- **Synthetic demo:** `/private/tmp/loo498-demo-c8knqzcq/lf-demo`, Task `30d671`,
  checkout `/private/tmp/loo498-demo-c8knqzcq/workspace-demo.30d671-try-the-rex`.
  Session `session:01a1a780-cf19-7b2f-a918-63ed79fa1bdc`, window
  `window:01a1a780-cf91-70d2-ae44-2ea526b4d33b`. Created Task view and one shell;
  repeat open reused both with zero LF Sessions. `client.inspect` returned a
  transient 404 after materialization; automatic display selection remains unproved.
- **Native Split demo:** session `session:01a1a560-b698-73a1-b2ea-7e56a5e8774d`,
  window `window:01a1a560-b699-7324-b0d0-efaf0ad2f01d`, original shell block
  `block:01a1a560-b698-7432-b8de-9e32f10e2da5`, label **LOO-498 split demo**.
  Expected directory `/Users/jack/src/loopflow.loo-498-open-and-restore`.
  This was created directly through Rex, not `lf open`. Its original prompt showed
  the checkout, but the app Split click was interrupted before any result.

## Next configured proof

Use a compatible current branch executable/store through the supported development
workflow, then demonstrate real `lf open`, repeat reuse, native splits, return/
reopen, and the agent-tool path in the configured Rex app. Test two Task tabs and
actual `pwd -P` after splitting. The raw API probe starts in HOME without cwd;
it does not establish whether the app's shortcut/menu supplies a directory.
No narrower split contract or deleted-checkout restoration requirement was accepted.

## November 5 — Split review (owed)

Use the normal Split shortcut or menu in Rex, then run `pwd -P` in the new shell.
Run this on a build of this branch, with a real Task.

1. `lf open --task <ID>`. Split from Task & workflow, Files, Changes, Conversation
   and Shell; each new shell should be in the Task checkout.
2. In the Shell pane, `cd /tmp`, then Split; the new shell should be in `/tmp`.
3. From a plain terminal in another directory, `lf resume <session-id>`, then Split;
   the new shell should be in the Task checkout.
4. Press Ctrl+], `cd` elsewhere, then Split; the new shell should follow the shell.

If a result is wrong, compare `rex block call -b <block> pwd` and `process` on
the source pane. Both agreeing with a wrong split means the app's report handling
is the next thing to inspect, starting with the OSC 7 host.
