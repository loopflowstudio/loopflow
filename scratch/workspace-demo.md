# LOO-498 opening repair — November 5, 2026

Jack Heart challenged stopping at the failed demo: “isnt that like literally what
this branch fixes”, then asked “you need to use a dev-lf to open i guess?”. The
operator is repairing the existing branch and replaying the real opening path;
no new Task, Graph, installation or merge is selected.

Two supported causes were found:

- `viewer_argv` used the installed-binary resolver on the real Machine. That
  resolver intentionally ignores LF_BIN for the normal account home. Consequently
  a branch open could launch the installed release for its observer/tool command.
  The opening now keeps its own executable on the real Machine; explicit private
  Machine runtime selection is preserved. `scripts/dev-lf` builds this checkout.
- Rex `client.list` includes the querying CLI itself. That process exits before
  `client.inspect`, producing the reproducible 404 and aborting display selection.
  Display selection now excludes CLI clients and inspects display-capable clients.

The earlier agent caller has no recorded terminal placement, and its inherited
REX_BLOCK is absent from the current native inventory. That is stale context, not
proof of failure for a currently attached branch viewer. Do not remove current-
viewer precedence or redirect a moved conversation to its startup pane to hide it.

The repaired real-Machine launch succeeded. The receipt reports Rex display
selection accepted and the existing conversation `connected`. A native screenshot
showed the selected “Open and restore a Loopflow workspace in Rex” tab displaying
this ongoing Codex conversation. No input takeover or provider restart was sent.
The 19 opening unit tests and 24 work_open integration tests passed (two optional
native fixtures ignored). Formatting and `cargo clippy --all-targets -- -D warnings` also passed.

## Successful replay evidence

- Durable evidence root:
  `/Users/jack/.local/share/loopflow/demos/loo498-20261105-opening-repair/data`
- Fixed copied runtime root:
  `/Users/jack/.local/share/loopflow/demos/loo498-20261105-opening-repair/runtime`
- Binary SHA-256: `9a8630ef53571b83650c77f2e22fd62133d06adc8b38aad53c465a0311279cc4`
- Build: `0.15.1+f12937190.dirty`, after the two repair edits; built from this
  checkout, copied before launch. The standard `scripts/dev-lf --version` build
  was followed by the focused test build containing the repairs.
- Real Machine: `LF_HOME=/Users/jack/.lf`.
- Launcher: `launch.sh`, started with Rex CLI in the Task checkout; native tab
  `window:01a1ad8e-3887-779d-911c-a7038ff9252c`, block
  `block:01a1ad8e-386d-7b04-895b-90cca83fdbe7`.
- Actual command: copied branch `lf open --task LOO-498 --host rex --json`.
- Created Task tab: `window:01a1ad8e-5a2b-7d1b-809b-6f3b9bda5d32`.
- Conversation block: `block:01a1ad8e-5a2a-74d2-a719-e4765a5f2f69`.
- Native container: `session:01a19d85-39a8-7aae-8c83-fa6adc2e1317`.
- Existing LF Session: `310fb7b6015843e498316cd921a348b9`.
- Files: `launcher.json`, `open.json`, `open.stderr` (empty), `view.json`,
  `observer-process.json`, `observer-pwd.json`, `observer-screen.txt`.
- The native process snapshot still shows login wrapper PID 57628. Its native
  directory report names the correct Task checkout; this is not an actual Split
  inheritance test. Do not signal that PID from historical evidence.
- The screenshot was observed in the computer-use tool result; no image file was
  saved. The retained terminal capture supplies textual evidence for the page.

The native Rex Split Right button was then clicked on the connected Conversation
pane. It created block `block:01a1ad91-9975-7bcb-827d-e3e4c64002a6`; native process
inspection reported foreground zsh PID 76928 with cwd exactly
`/Users/jack/src/loopflow.loo-498-open-and-restore`. A screenshot showed the
conversation left and the new shell right. This proves this native Conversation →
Split cwd case, beyond just a title or directory report. No shell input was sent.
Evidence: `after-split.json`, `split-process.json`, `split-screen.txt`.
The mirrored conversation showed text reflow artifacts after splitting; readability
under resizing remains an observed limitation, not accepted visual fidelity.

This repairs the direct-terminal opening path against real Work and a live owner.
It does not claim Split from every pane, cd-then-Split, remote Machines, cmux,
human input transfer, or agent opening after viewer movement were demonstrated. The existing saved layout
contains only Conversation, so zero new shells is expected. No new layout choice
or full-feature acceptance was requested or inferred.

Retain both roots and all earlier roots. No input was sent to the conversation or
launcher shell, and no user input settlement or cleanup approval was received.
Session history remains in the real Machine. After input settlement and fresh
identity checks, only these two verified native tabs may be closed; confirm their
owners exited before removing this runtime root. Never remove the evidence root.
The existing conversation can be reopened with installed `lf resume
310fb7b6015843e498316cd921a348b9` through Rex CLI. Closing a view is not Task completion.

The following failed demonstrations remain dated evidence, superseded only for
the successful direct-terminal opening replay described above.

---

# LOO-498 actual workspace opening — November 5, 2026 retry

**Result: failed configured demonstration.** The Task remains at feature → demo;
PR #1814 is open. No behavior acceptance, implementation restart, installation,
shared-store migration or shipping approval follows from this review.

Jack Heart responded “whit what” and “try again” after the manually assembled
three-pane demonstration below. Interpretation: that demonstration did not show
the requested workspace-opening feature clearly. It is superseded as the current
review; its terminal-tool evidence remains useful.

The retry used Rex CLI to create a launcher terminal, then ran the copied branch
binary's `open --task LOO-498 --host rex --json` inside that actual Rex terminal,
using the real Machine (`LF_HOME=/Users/jack/.lf`). The fresh direct-terminal
launcher removed inherited LF agent markers before setting its explicit LF_HOME
and LF_BIN. This tests the direct-terminal path, not the earlier failed agent path.
Current design: [rex-workspace.md](rex-workspace.md). Guide:
[docs/workspaces.md](../docs/workspaces.md). Review: [demo.html](demo.html).

## Observed result

- The receipt reports creation of a fresh Task tab from the saved layout, with
  zero new shells. This real Task's saved layout selects a Conversation pane.
- Display selection failed: `Rex client.inspect` returned 404 `client not found`
  for `client:01a1ad6f-8c21-758d-b59a-951265a004de`.
- Conversation readiness remained `uncertain`: “Observer has not reported an
  attachment”. This does not prove provider continuation or a usable viewer.
- An initial native inventory listed the created tab. The subsequent saved
  `after-actual-open.json` inventory does not contain it or the launcher tab.
  The cause of their absence is unknown; no attribution to Jack or Rex is made.
- No blind recreation, terminal input, provider takeover or cleanup followed.
  No before/after screenshot or native Split result was obtained.

## Exact evidence and retained runtime

Both roots from the earlier tools demo remain fixed and intact:

- Evidence: `/Users/jack/.local/share/loopflow/demos/loo498-20261105-live-tools/data`
- Runtime: `/Users/jack/.local/share/loopflow/demos/loo498-20261105-live-tools/runtime`
- Binary: runtime `lf`; SHA-256
  `8ead6244eea91e198fac59e4db77e43bff7e028f5ed204eb5d71bc36c1e958fb`.
- Launcher: runtime `launch-demo.sh`; tab
  `window:01a1ad6f-2374-7046-a5d8-3dc5b7982bd0`, block
  `block:01a1ad6f-2363-71c5-b86c-49a1b4aca192`.
- Created destination: `window:01a1ad6f-8b60-7340-9e1c-a20a19c8ddc6`.
- Rex container: `session:01a19d85-39a8-7aae-8c83-fa6adc2e1317`.
- Existing LF conversation: `310fb7b6015843e498316cd921a348b9`.
- Evidence files: `actual-open.json`, `actual-open.stderr` (empty when read),
  `after-actual-open.json`. The launcher retained its shell after printing results.

The roots retain evidence and runtime, not an isolated Machine. Live Session
history remains in `/Users/jack/.lf`. No pending input has been declared settled;
absence from native inventory is no teardown authorization. Preserve these and all
earlier roots. The earlier exact-owner teardown procedure remains conditional on
input settlement and fresh native/process identity checks; do not reuse old PIDs.

## Next action and review boundary

Recommend repairing the real opening path before another demo: determine why
selection targets an unavailable Rex client, inspect the launched observer's
readiness, and explain the disappearing native destination from fresh evidence.
Then repeat direct-terminal and agent-caller opens, preserving the original
conversation and native arrangements, followed by native Split/cwd verification.
The accepted fresh-launch/config/Desktop-layout ownership design is unchanged.
No implementation Graph was started during this failed review. The current page
records the failure; the earlier manual layout is not offered as successful proof.

---

# Earlier manually assembled tools demonstration (superseded)

# LOO-498 live Task tools — November 5, 2026, resumed demo

Jack Heart invoked `$demo` again after the LOO-717 handoff. His instruction to
open through Rex CLI until this LF lands remains in force. The demo now has one
real tab, **LOO-498 · Workspace demo**, with Task & workflow on the left and Shell
and Changes on the right. It was manually arranged through Rex CLI, not `lf open`.
Current design: [rex-workspace.md](rex-workspace.md); guide:
[docs/workspaces.md](../docs/workspaces.md); runbook: [demo.html](demo.html).

## Observed and still unproved

- Rex creation receipts confirm three native blocks in one tab. Background terminal
  captures show the Task brief and navigation footer, a zsh prompt, and the real Git
  diff in Changes. Rex process reads report all three in the actual Task checkout.
- Task view shows the original stored brief, including superseded reopen/idempotence
  wording. This is a documentation/Work-state defect, not the accepted fresh-launch
  contract. Do not treat that displayed text as new direction.
- The Task pane's native `pwd` method returned `{}`; its process cwd was correct.
  This does not prove what Rex's native Split action will inherit.
- Trying to select the new tab via UI was interrupted by a user app change. A fresh
  UI read showed another Task selected. No more focus/input actions were sent there.
  The demo was completed in the background, preserving Jack's selection. Native
  before/after screenshots and Split proof therefore remain absent.
- The earlier configured agent-caller `lf open` failure remains a failure. Manual
  Rex creation does not prove source-layout selection, repeat-launch behavior,
  automatic arrivals or the branch's opening resolver. No replacement provider,
  conversation, Task, or Graph was created for this tools demo.

## Fixed runtime and retained evidence

This is a requested live-data tools demonstration on Jack's existing Machine,
not a disposable private-Machine fixture. No shared-store migration or installation
was performed. The copied binary is held outside rebuildable output.

- Durable evidence: `/Users/jack/.local/share/loopflow/demos/loo498-20261105-live-tools/data`
- Runtime: `/Users/jack/.local/share/loopflow/demos/loo498-20261105-live-tools/runtime`
- `LF_HOME=/Users/jack/.lf` (existing real Machine)
- `LF_BIN=/Users/jack/.local/share/loopflow/demos/loo498-20261105-live-tools/runtime/lf`
- Binary SHA-256: `8ead6244eea91e198fac59e4db77e43bff7e028f5ed204eb5d71bc36c1e958fb`
- Rex session: `session:01a19d85-39a8-7aae-8c83-fa6adc2e1317`
- Tab: `window:01a1ad61-d395-7add-bd98-2246f32a050d`
- Task block: `block:01a1ad61-d381-74ed-b8d6-8ea0f002e460`, observed lf PID 54545
- Shell block: `block:01a1ad62-5993-7e5f-a6e1-bf4ba4496624`, observed zsh PID 6402
- Changes block: `block:01a1ad62-830e-708c-9bcc-81d6e0b26107`, observed lf PID 21932
- Rex server PID 71053; instance `0566016a-29cf-4cc1-993f-ee1d6ea081d5`;
  host `6cd4509a-df69-4a66-88f3-53eaa4d8523e`.
- Evidence: `launch.json`, `*-pane.json`, `*-process.json`, `*-screen.txt` in the durable root.

These PID readings are evidence, not future signal authority. The operator sent
no terminal input. Jack's future shell input/drafts have not been settled; retain
all blocks and both roots. After explicit input settlement and fresh identity
checks, close only this demo tab with
`rex window close window:01a1ad61-d395-7add-bd98-2246f32a050d`, confirm its owners
have exited, then remove only the runtime root. Never remove the evidence root or
any earlier demo roots. Real Session history remains in `/Users/jack/.lf`; the
existing Task conversation is `310fb7b6015843e498316cd921a348b9` and can be resumed
with the installed `lf resume 310fb7b6015843e498316cd921a348b9` through Rex CLI.

## Earlier review instructions (destination no longer observed)

Select **LOO-498 · Workspace demo**. Use the Task pane's native Split Right control,
then run `pwd -P` in the new shell: expected
`/Users/jack/src/loopflow.loo-498-open-and-restore`. In the existing shell, `cd /tmp`,
then split that shell: expected `/tmp`. Inspect Changes with the normal pager
controls; Task view uses `n`/`p` for steps and `l` for a log.

No human observations or acceptance have been received for this resumed demo.
The recommended next action is that native Split/cwd review, then repair the
configured opening failure and stale Task brief in a caller-selected implementation
pass. No implementation Graph was launched during this review.

---

# LOO-498 configured opening and LOO-717 handoff — November 5, 2026

Jack Heart invoked `$demo`, then redirected the interaction to understanding and
opening LOO-717 and sending it all relevant context. Jack instructed: “you have
to use rex's CLI to open things until this lf lands.” Use Rex CLI for host opening
until landing; no further branch `lf open` launch was attempted after that instruction.
Current design: [rex-workspace.md](rex-workspace.md). Guide:
[docs/workspaces.md](../docs/workspaces.md). Runbook: [demo.html](demo.html).

## Observed result and feedback

Before that instruction arrived, the branch binary attempted
`lf open --task LOO-717 --host rex --json`. It failed before creation:
“Current conversation viewer is unavailable; refusing stale Rex startup placement”.
The receipt returned `destination: null`, `container: null`, `status: failed`.
This configured agent-caller path remains unproved and currently fails; the passing
isolated native explicit-host test does not cover it. No duplicate tab was created.
This is a new concrete finding for a later implementation pass, not demo acceptance.

Rex CLI then successfully ran the installed LF resume command in a fresh tab,
using `/Users/jack/src/loopflow.loo-717-local-and-remote` as cwd. Rex capture showed
the existing LOO-717 Codex conversation resumed and working in that directory;
LF Session inventory confirmed it active. This demonstrates the direct Rex handoff,
not the branch workspace launcher. No native app Split, repeated-launch comparison,
before/after UI screenshots, or LOO-498 behavior acceptance was obtained.

- Rex session: `session:01a19d85-39a8-7aae-8c83-fa6adc2e1317`
- New tab: `window:01a1ad54-195c-7728-a8ae-75cbb3b3a174`
- New block: `block:01a1ad54-1928-7c9a-be70-22af96eb7e50`
- LF Session: `a1ef4210816449a0a354caf02d3904cc`
- Task handoff comment: `3896fecd-adcf-4a5f-9757-1555b49734a3`
- Conversation steer receipt: `75c5149f-be54-45ce-92f5-c7878b9c6efa`
- Full delivered context: [loo717-handoff.md](loo717-handoff.md)

The handoff states accepted no-host-persistence ownership, the existing cleanup API
conflict, retained-terminal protection requirement, native API evidence, current PR
and tests, remote-arrival behavior, and missing configured proof. It does not approve
a new Graph, shipping, installation, or shared-store migration. The existing
conversation may operate its already-authorized Task; this handoff launches no Graph.

This is an ordinary requested opening in Jack's real Rex session, not a disposable
fixture. No private Machine/runtime roots were created. Leave the conversation and
its native tab intact. Earlier demo roots below remain retained; no draft release
or teardown authorization was received.

## Recommended next action

Continue the LOO-717 discussion in its now-open conversation. For LOO-498, preserve
the configured viewer-unavailable failure for implementation review; do not treat
Rex CLI success as proof that `lf open` is fixed for this live caller. Resume the
workspace demo only as directed, using Rex CLI to open hosts until this LF lands.
There is no new design acceptance beyond the explicit CLI-opening instruction.

---

# Earlier demo evidence (preserved)

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
