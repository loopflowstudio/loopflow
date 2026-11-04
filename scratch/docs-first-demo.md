# LOO-728 docs-first demo — November 4, 2026

Decision pass 1: **iterate**. The configured experience failed. This is the
agent's decision under the headless delegation, not observed acceptance by
Jack Heart. No new product choice or scope reduction is proposed.

Design: [write-docs-first.md](write-docs-first.md). Integration under review:
`f20e05d1eb027ed435988b35c5a936f7effaa349`, PR #1817, stacked on LOO-498.
The preceding pass integrated and published the preserved implementation and
passed focused checks. It explicitly left configured-host acceptance unverified.
This pass adds real failure evidence; missing earlier demo evidence does not
establish a stalled implementation.

## Configured-path observations

The existing Rex server was reachable without autostart. Its existing
`loopflow` Session was `session:01a19d85-39a8-7aae-8c83-fa6adc2e1317`.
The current window was `window:01a1a819-d002-7cb2-8ee8-713b924e42ba`,
labelled `LOO-728 Docs first`; focused block
`block:01a1a819-d001-794e-90aa-967b912ab9b5` remained focused after presentation.
Task inventory identified the active interactive conversation as
`fd5f2333c5f54054b40c4bb5e574a98a`. A window label alone does not prove that
conversation's current viewer registration.

Commands used the already-built `target/debug/lf`, reporting
`0.15.1+13e7d5dac.dirty`. This is the preceding validation artifact, not a fresh
build at the review HEAD. Installed `lf` reported `0.15.1+ac8e088f5`.

1. `target/debug/lf open --task LOO-728 --file docs/orchestration.md --file
   docs/surfaces.md --explain --json` selected Rex and the two original files
   under this Task checkout, with `opening:false`.
2. The same command without `--explain`, retaining this agent's caller
   identity, exited 1: `Current conversation viewer is unavailable; refusing
   stale Rex startup placement`. No file pane was created by this attempt.
3. As a diagnostic only, removing `LF_AGENT_CALLER` from that command produced
   `opening:true` and two new native file panes in the existing window, without
   changing its focused block. This uses direct-shell placement and does **not**
   prove the required agent route.
4. `rex --autostart=false capture -b <block> --trim` on both new panes showed
   `error: unrecognized subcommand 'tool'` followed by `Usage: lf task
   [OPTIONS] <COMMAND>`. The panes were
   `block:01a1a845-fb5d-702a-b036-8d00184436b4` (orchestration) and
   `block:01a1a845-fc74-7631-a6fc-5de8277bf5e6` (surfaces). Neither displayed
   a working editor. Installed `lf help task tool` independently failed with
   `unknown command: lf task tool`.
5. A direct-shell retry again reported `opening:true`; its panes also did not
   persist. This establishes neither editor readiness nor successful reuse.
6. Presenting `docs/loo-728-does-not-exist.md` failed with `Cannot open ...`
   and `No such file or directory`. `git diff --exit-code --
   docs/orchestration.md docs/surfaces.md` passed: the original files remained
   unchanged and recoverable.

The launch mismatch is consistent with `workspace::viewer_argv` using
`os_process::resolve_lf_binary`, which prefers an installed entry, while this
unreleased stack introduces `task tool`. Isolated tests pin `LF_BIN` in a
private Home and therefore do not establish this live installation path.
The precise installed entry used by each exited pane was not captured.

Cleanup inspection found neither demo file pane remaining. An attempted close
of their original IDs reported that the blocks were already gone. A final
window inventory showed only the original focused block. Another pre-existing
shell also disappeared during the observations; this demo issued no close or
input command to that shell, and its disappearance is not evidence of preservation
or approval. No server, watcher, provider or editor was left running by this demo.

## Remaining proof and next action

Implementation should repair or configure the existing launch path so a
docs-writing agent can present original files in the Task's current Rex viewer
and the resulting Files tool executes from a compatible build. Investigate
current-viewer registration/selection and executable resolution together;
do not make stale startup placement authoritative or bypass the failure by
erasing agent identity. Use the existing installation and Files owners, without
landing the parent or adding a competing host abstraction.

Repeat the unmodified agent command and capture the editor displaying the
original documentation before advancing. Then verify an unsaved active edit
survives repeated presentation and that a saved correction is read by the next
implementation pass. Those checks could not run because no working editor
opened. Desktop draft behavior retains its prior automated evidence, not live
acceptance. The pursue source still begins with write-docs and both backward
edges target implement; this pass did not run a new implementation Graph.

Jack Heart supplied no new feedback during this headless demo. His accepted
in-place docs and current-host requirements supply the decision: a success
receipt followed by a failed editor does not satisfy them. The gaps are concrete
and repairable, so iterate rather than request another product decision.

## Repair evidence — November 4, 2026

The implementation pass found that the Task's interactive attachment is inside
its retained execution PTY, while Rex hosts the outer relay. Its recorded
`terminal_id` is null. Placement now travels with the transport's controlling
viewer and is read through the existing Session summary owner. Local Files
presentation uses the shared editor command directly; it no longer depends on
an installed `lf` supporting this branch's `task tool` command.

A headless probe used the real Rex binary with a private socket, config, state,
LF Home and repository, a candidate execution owner, its actual Rex relay, and
agent provenance. It opened the repository's original `guide.md` in vi, entered
an unsaved correction, repeated the same file presentation, and confirmed the
same editor block, original focus and unsaved text. Saving changed the original
file; rereading returned the correction. A missing-file request failed without
changing that saved content. All probe Sessions and process groups were closed.
No provider, production store or existing pane was altered.

This proves the candidate's native agent/editor path headlessly. It does not
upgrade the existing configured owner or establish Jack Heart's live acceptance.
That owner predates placement reporting; the configured demo still needs the
candidate owner and viewer through the normal delivery path. The original
failure above remains evidence for that boundary, not a reason to restore stale
environment routing or restart an active conversation during implementation.
