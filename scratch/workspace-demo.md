# LOO-498 workspace demo — November 3, 2026

Jack Heart invoked `$demo` in the primary conversation. This review is outside
the stopped feature Graph; it does not resume, approve or complete that Graph.
The guide is [docs/workspaces.md](../docs/workspaces.md), the design is
[rex-workspace.md](rex-workspace.md), and the interactive runbook is
[demo.html](demo.html).

## Observed through the configured path

- Inspected the installed `/Applications/Rex Beta.app` through native accessibility
  and captured its before screenshot in the conversation. The actual app reports
  Rex Beta 0.1.0 (1696), connected to the configured local server.
- Attempted the branch command against the real Task and configured server:
  `./target/debug/lf open --host rex --task LOO-498 --copy 'LOO-498 demo' --new-shells --json`.
  It failed before workspace creation with a database migration-history/schema
  mismatch. Binary identity: `lf 0.15.1+af774cada.dirty`; it reported latest known
  migration `wave_removal` and last store writer `lf 0.15.0 (ce607b9fc)`.
  Installed `lf --version` reports 0.15.1. The local binary predates current HEAD
  `3d61147152`; no fresh build or compatible development-store setup was established
  in this demo. No installation promotion or live-store migration was attempted.
- No LF workspace opening, reuse, live observation, or agent-path success is
  established by this failed invocation. Earlier isolated tests remain separate
  evidence and do not substitute for the configured-path demonstration.
- Created a separately named native Rex session through its public API, solely
  to test the Split UI. This was **not** created by `lf open` and is not evidence
  that the LF feature works. Its single shell starts in the LOO-498 checkout;
  the app showed the correct worktree and branch in its prompt.
- Selected that demo session through Rex's advertised `session.select` client
  action. The normal Split Right button was visible. Before the click completed,
  computer use reported that Jack had changed the app; the next read showed
  another Task. No further input was sent and no split result was obtained.
  The native app Split path remains unverified. There is no after screenshot of
  a successful LF opening or successful native split; neither was fabricated.

## Demo workspace left available

- Session label: **LOO-498 split demo**
- Tab: **Task worktree**
- Session: `session:01a1a560-b698-73a1-b2ea-7e56a5e8774d`
- Window: `window:01a1a560-b699-7324-b0d0-efaf0ad2f01d`
- Original shell block: `block:01a1a560-b698-7432-b8de-9e32f10e2da5`
- Expected directory: `/Users/jack/src/loopflow.loo-498-open-and-restore`

This contains a fresh native shell only. No provider was launched or moved, and
no prior user pane was replaced or closed. The demo session remains available
for Jack to try the normal Split button and run `pwd -P` in the resulting shell.

## Interpretation and next action

The raw `session.new_split` probe establishes HOME fallback when creation omits
cwd. It does **not**, by itself, establish whether the app's Split shortcut/menu
supplies the source directory. Earlier statements that the entire native host
requirement is impossible were broader than that evidence. Preserve the probe,
but verify the app action before proposing a scope reduction or host dependency.

First establish a compatible current branch demo executable/store through the
repository's supported development workflow. Then demonstrate actual `lf open`,
repeat reuse, native splits and the LLM tool path in the configured Rex app.
Use the dedicated demo session for the native Split observation in the meantime.
No product revision, narrower acceptance, shipping or merge was approved.

Jack's feedback so far: requested the demo; no observed-behavior acceptance or
native Split result has been supplied. The review remains available here.
