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
