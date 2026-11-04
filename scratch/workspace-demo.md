# LOO-498 workspace demo — November 3, 2026

Jack Heart invoked `$demo` in the primary conversation. This review is outside
the stopped feature Graph; it does not resume, approve or complete that Graph.
The guide is [docs/workspaces.md](../docs/workspaces.md), the design is
[rex-workspace.md](rex-workspace.md), and the interactive runbook is
[demo.html](demo.html).

## Disposable database demo — November 4, 2026

### Dev stream compatibility clarification

Jack Heart subsequently said synthetic Tasks are insufficient to evaluate the
feature: review needs his real Tasks, Waves and conversations. The disposable
demo is therefore supporting evidence only. He explicitly requested switching
local LF to the dev stream. That authorizes the installation and schema update,
not landing LOO-498 or treating the fake-data demo as accepted.

Installation outcome, November 4: the original `lf install --dev` succeeded
for the CLI, selected `ac8e088f5`, recorded stream `dev`, and migrated the
shared database to `wave_removal`. The explicit `~/.local/bin/lf` now returns
`0.15.1+ac8e088f5`; its preflight reports exact schema compatibility and
`promote`, and Task/Session inventory reads succeed. The existing scheduled
installer remains registered. No LOO-498 Graph was restarted.

The older installer did not update Desktop. This conversation inherits
`LF_BIN=/Applications/Loopflow.app/Contents/MacOS/lf` and finds that old bundled
CLI first, so bare `lf` here still reports 0.15.1 and cannot read the migrated
store. A second install through the new explicit CLI downloaded the matching
Desktop package, but refused replacement: executable identity could not be
proved for PID 43082. Inspection found twenty app-named helpers without readable
kernel executable paths; PID 43082's mapped executable is an older app in Trash.
The supported rollover dry run includes many other conversations, including
some reported in-progress turns whose sockets do not answer. It was not applied;
no helpers were manually stopped. Desktop remains old and incompatible with
the upgraded store. Repairing that update boundary is still required; the CLI
switch must not be described as successful Desktop installation.

Jack Heart asked whether moving to dev was safe and said the explanation was
unclear. Streams select installed binaries; the database stays in place and
may migrate during installation. The unmerged LOO-498 branch is not the dev
stream, and its failed catalog does not establish dev incompatibility.

The newest successful main package inspected on November 4 was GitHub run
`41774946128`, revision `ac8e088f5f4d18db1fdb5d7c4e7685c86ec9a004`.
Its downloaded CLI (`0.15.1+ac8e088f5`) passed `self install preflight --json`
against `/Users/jack/.lf/loopflow.db`: `promote_and_migrate`, no unmerged
migrations, executable compatibility with 19 references, and no active runs
reported by preflight. It skipped 31 references whose worktree catalog roots
were gone. The reader floor would advance from the applied `task_regressed_in`
frontier to `wave_removal`; older builds may then remain unavailable until
their stream catches up. This proves the checked compatibility path, not the
absence of dev regressions. No installation ran during that preflight; the
subsequent authorized dev installation is recorded above.

Jack Heart requested a fake database to continue the demo. A fresh private
`LF_HOME` now uses this branch's schema, with one synthetic Task created through
`lf task create` and placed through `lf task checkout`. No production records
were copied. This bypasses the migration blocker for feature review; it does not
prove upgrade compatibility.

- Private Home: `/private/tmp/loo498-demo-c8knqzcq/lf`.
- Demo Task: `30d671`, **Try the Rex workspace**.
- Checkout: `/private/tmp/loo498-demo-c8knqzcq/workspace-demo.30d671-try-the-rex`.
- Run commands using `/private/tmp/loo498-demo-c8knqzcq/lf-demo`; it pins the
  branch executable, private Home and fixture repository.
- `lf open --host rex --task 30d671 --copy 'LOO-498 disposable demo' --new-shells`
  created a native Task & workflow pane and one shell. Rex terminal capture
  shows the Task brief and recorded checkout, and the shell prompt shows the
  demo worktree and branch. The real LF store was not migrated.
- Repeating with `--no-focus` reused window
  `window:01a1a780-cf91-70d2-ae44-2ea526b4d33b`, created zero shells and returned
  the identical native layout. The private store has zero LF Sessions; no
  provider or Task Graph was launched.
- Rex session: `session:01a1a780-cf19-7b2f-a918-63ed79fa1bdc`, sidebar label
  **Try the Rex workspace · 30d671 · LOO-498 disposable demo** (plus LF marker).
- Opening exposed a separate display-selection failure: `client.inspect`
  reported a transient client missing (404). The native session/tab still
  exists and is visible in the app sidebar. A click to select it was interrupted
  when Jack switched to another Task. No subsequent input was sent there.
  Automatic selection remains broken; no successful app-after screenshot exists.
- Native Split UI, unfinished-input retention and actual LLM routing remain
  unverified. Jack has not accepted observed behavior yet.

The synthetic demo remains in Rex's sidebar as supporting evidence. Jack's
later review direction requires real data. No installation was performed as
part of creating that synthetic demo.

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

### Schema diagnosis after the demo

Jack Heart asked whether publishing a release and using Preview would resolve
the mismatch. A clean-source rebuild at `da41d2031` still fails the read-only
installation preflight. The live store's latest migration sequence is
`0.15.0.001_release → ship_stop → task_regressed_in`. The generated branch
catalog instead orders the remaining slugs alphabetically, including
`rex_workspaces` before `ship_stop`, and marks all five slugs unmerged.
Git's first-parent `origin/main` history contains the four mainline slugs in
their proper order; why build-time discovery loses that evidence remains
unresolved. This is not explained by executable age alone.

Preview selects the newest published release automatically. A release is not
yet a demonstrated repair for the local catalog problem, and the branch also
contains its unmerged workspace migration. No release, installation promotion
or live database migration was performed. Fresh Task status shows PR #1814
open and the feature Graph stopped, with no live Graph process.

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
