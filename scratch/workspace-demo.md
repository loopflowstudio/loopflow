# LOO-498 — failed demo and corrected launch design

## November 5, 2026: observed failure and Jack Heart's direction

The branch repeatedly opened LOO-498's active conversation as a custom passive
`lf session view`. Jack reported “everythime you launch a demo this pane freezes”.
Source inspection found ignored input and a hidden Ctrl+R transfer shortcut. A
read-only hint was added in `21bbb00329`; its three PTY checks passed, but that did
not establish the cause of the reported freeze.

The subsequent `$demo` built that commit and opened a fresh Rex tab through Rex's
CLI using the real LF Machine. It reported connected. Both the native screenshot
and Jack's supplied screenshot showed overlapping text, stale prompt fragments,
and inconsistent spacing. Jack said “still ooks super werid”, then explicitly
said to leave this Task alone because he was using it. No acceptance was given.
The operator stopped host interactions; no input transfer or teardown occurred.

Jack clarified the expected behavior: launch a Rex tab in the right worktree and
run ordinary `lf resume`. He then requested dramatic simplification of the code
and questioned unnecessary Session commands. Current implementation direction is
[rex-workspace.md](rex-workspace.md): remove the observer command, readiness
files/protocol, and custom input transfer. Preserve ordinary resume, layout and
worktree handling. Do not use this active Task or any of its panes for a demo.
Further hands-on proof needs separate work; no live demonstration is accepted.

## Last failed opening — retain these resources

- Commit: `21bbb00329`; runtime SHA-256:
  `70b984d899ff1b10cc004c6d356f0e00fe25b47121394b2ee95b0c91e0548cec`.
- Evidence root: `/Users/jack/.local/share/loopflow/demos/loo498-20261105-input-hint/data`.
- Fixed runtime: `/Users/jack/.local/share/loopflow/demos/loo498-20261105-input-hint/runtime`.
- LF_HOME: `/Users/jack/.lf`; runtime `lf` is LF_BIN. This used real data, not a
  private Machine. The runtime is a copy, not an installed update.
- LF Session: `310fb7b6015843e498316cd921a348b9`.
- Rex session: `session:01a19d85-39a8-7aae-8c83-fa6adc2e1317`.
- Launcher tab: `window:01a1aeec-835a-7b56-b977-8e2d862a70a1`;
  block `block:01a1aeec-834d-70a0-9cc5-892cc17febdc`.
- Conversation tab: `window:01a1aeec-98d3-79fa-accd-f2cc7aa0838d`;
  block `block:01a1aeec-98d2-75d4-9958-b130d347fb40`.
- Evidence: `before.json`, `after.json`, `launcher.json`, `open.json`,
  `open.stderr`; screenshots are in the conversation, not saved image files.
- The earlier tab `window:01a1adc5-3ed3-7cec-b32d-f640c130b637` was preserved.
  Its observed shell was `block:01a1ade8-92ba-7626-9e1d-79a772f4d43d`.

These are historical identities, not current process liveness or signal authority.
No draft/request settlement or disposal authorization exists. Leave all resources
untouched. Do not infer release from absent blocks, finished providers or age.

## Earlier retained roots and evidence

Under `/Users/jack/.local/share/loopflow/demos/`, retain both `data` and `runtime`
for each of:

- `loo498-20261105-review`: same real Machine; saved Desktop layout was one
  Conversation pane. Native receipt proved connection, not usability.
- `loo498-20261105-opening-repair`: fixed child executable selection and transient
  Rex CLI discovery. Native Split started zsh in the Task checkout; text reflow
  was visibly broken. Repairs were published in `1a788c6a6f`, synced `e9dcba147b`.
- `loo498-20261105-live-tools`: manual tool panes and a mixed-binary failed opening.
- `loo498-20261105`: private real-data snapshot at `data/lf`; its original native
  session later disappeared. This is not evidence that all input was settled.

Retain `/private/tmp/loo498-real-demo-j8rn_nmv` and
`/private/tmp/loo498-demo-c8knqzcq` too. The latter's synthetic Task was `30d671`.
Earlier exact receipts, owner identities, safe conditional teardown and history
reopening instructions are in `21bbb00329:scratch/workspace-demo.md`. They require
fresh ownership evidence and input settlement before use. Never delete durable
history or stop processes by name. User's current instruction is to leave the
active Task and its native panes alone.

## Remaining proof

Use isolated host/config/Machine fixtures to establish native tab creation,
ordinary resume, provider/draft continuity and cwd. Keep generic resume screen
logic unchanged. Update [demo.html](demo.html) last with the resulting evidence;
no new live demo, installation, Graph launch or merge is selected here.

## Simplification proof

`441ed217fd` removed the observer path (916 net Rust lines); `8c77f061a4`
synced current main without conflicts. Isolated Rex tests now prove ordinary
workspace resume keeps the same provider and unfinished input, with the Task's
working directory. The separate manual-resume cwd test also passes. No running
user Task, native pane, installed binary or shared database was used for this
validation. Layout/config/host behavior and the independent cwd repair remain.

Checks: opening library 14 passed; normal-viewer library 8 passed; work_open
integration 22 passed initially, its corrected three-pane shell selection test
passed separately, and its unchanged tmux test passed standalone after an initial
2-second host timeout; isolated native Rex 2 passed. No timeout relaxation or
retry loop was added. `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.

The review page describes the simplified flow without asking Jack to reopen his
active conversation. Hands-on acceptance is still outstanding and must use
separate work. No Graph continuation, merge or teardown was selected.


## November 6, 2026: separate native demo — conversation startup failed

Jack Heart invoked demo after the simplification and user-documentation rewrite.
The operator used a separate Task, 99543b (task_99543b5a41a84b7cb6e76f5d625a52ef),
in `/Users/jack/src/loopflow.rex-native-demo`. No existing conversation was resumed,
closed or modified. Task creation and checkout launched no Graph. The demo brief
prohibits implementation, Graph execution and other-Task operation.

The copied branch CLI at commit 21ae187407 read the real Machine successfully.
A Rex CLI-created launcher ran `lf open --task 99543b --json`, without a host flag.
Rex was detected and the fresh tab contained Conversation, Task & workflow and
Shell. The conversation block reported the demo checkout through OSC 7. This is
not proof of a manually created split's actual cwd.

Conversation startup then failed: the pane showed “Nothing to resume here;
starting a new conversation”, then disappeared. The Task view and shell remain
visible and readable. Native screenshot evidence is in this conversation; no
filesystem screenshot export was available. No provider input or split inheritance
was accepted by Jack. The initial opening receipt is not evidence of usability.

Process and history evidence identified a mixed runtime: the branch outer resume
was d1181700379fcf6a97efa8cffc3ab41d2a1e233e4053ba5159fd983bb682cde5;
`ops/human_session.rs` selects `resolve_lf_binary` for its child, which on a real
Machine selects the installed executable. Its frozen runtime was
`~/.lf/bin/lf-e497ed1b11a25358a978332cb156d9428dd53436aa67d07d048d6edeec5d76eb`
(version 0.15.1+5519340b4). The child recorded failed at 16:14:05 UTC; the LF log
says “execution owner did not take the provider wait; waiting here” and “Execution
owner closed before answering”. The precise cause of provider exit remains unknown;
the runtime mismatch is established, not proven causal. Recommended next work:
trace that ordinary-resume exit and ensure configured branch demos use one runtime
through the whole launch. Do not restore a custom viewer or add Session commands.

### Retained evidence and owners

- Durable evidence: `/Users/jack/.local/share/loopflow/demos/loo498-20261106-native/data`.
- Fixed runtime: `/Users/jack/.local/share/loopflow/demos/loo498-20261106-native/runtime`.
- Real `LF_HOME=/Users/jack/.lf`, per Jack's prior real-data preference; no private
  Machine, installation or database migration. Both roots remain intact.
- LF Session: `efb749e7cffb42bc88ff86a25575e1c6`; final Session list says closed.
- Rex session: `session:01a19d85-39a8-7aae-8c83-fa6adc2e1317`.
- Launcher: window `01a1b1a2-907f-7697-b4aa-f2c77e55d23c`,
  block `01a1b1a2-9063-769f-a929-909716148b2a` (both retain their Rex prefixes).
- Demo tab: `window:01a1b1a2-ad99-7dcf-b4bf-e9e3e6056a3d`.
- Removed Conversation block: `block:01a1b1a2-ad92-73c8-aa90-6b99fddaa172`.
- Retained Task view: `block:01a1b1a2-ad94-7582-a218-8515cde088d8`;
  PID 77240, parent login 77152, started November 6 at 08:13:11 local.
- Retained shell: `block:01a1b1a2-ad99-749b-a66f-020a7c96ce6c`, login 77153.
- Conversation owners 77150, 77242, 78145 and 78445 no longer appeared in `ps`.
  They are historical identities, never future signal authority.
- Evidence files: create.json, checkout.json, before.json, launcher.json, open.json,
  open.stderr, after.json, after-exit.json, history.json, sessions-final.json,
  ps-final.json. Input artifact: `~/.lf/runs/f5/f5acc3d427ec456bb81cb9e7eae3650f`.
- History remains readable using `lf session history efb749e7cffb42bc88ff86a25575e1c6
  --json` against the real Machine. No usable provider history was demonstrated.
- No input settlement or teardown authorization was received. Leave native shells
  and roots intact. After Jack answers the runbook's settlement question, recheck
  the exact retained owner identities before closing demo-owned tools. Use `q`
  in Task view and `exit` in demo shells; do not signal by name. Never remove the
  real LF_HOME, checkout, or durable history. Runtime removal is deferred until
  every owner is verified exited and all input is settled.

LOO-498 remains feature → demo; no live Graph was observed. The branch was synced
with main c8d7a7f4a8; its AGENTS conflict kept main's prose reduction and this Task's
isolated-host testing rule (864778108a, pushed). The demonstrated binary predates
that documentation/skill-only merge. No Rust change or repeated build was needed.
The runbook is `scratch/demo.html`; this review is failed, not accepted.
