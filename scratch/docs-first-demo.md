# LOO-728 configured acceptance — November 4, 2026

Configured acceptance remains unproved. The latest implementation diagnosis
supersedes earlier instructions to keep patching viewer routing: migration order
is repaired, but the candidate still requires parent delivery before supported
installation and same-Session owner transition. Retain Jack Heart's accepted
current-host design; the Desktop-only proposal is unaccepted. This is delegated
judgment, not new feedback or acceptance from Jack.

## Delivery prerequisite

The candidate initially rejected the shared store because shallow Git history
made `migration_build::merged_order` return no merged order. Its slug tail was
`native_terminal_input, rex_workspaces, ship_stop, task_regressed_in, wave_removal`.
Read-only ledger inspection found `ship_stop, task_regressed_in,
native_terminal_input, wave_removal`, with matching source SQL checksums.

`git fetch --unshallow origin` completed and rebuilding with inherited `LF_*`
unset succeeded. The catalog now follows the ledger, then `rex_workspaces` as
its sole unmerged migration. No source-code or shared-store change was required.
PR #1814 was freshly observed OPEN at `b8aac2ef139ff34213685a10137572e1498f5e4f`
with no merge timestamp during that diagnosis. `install::decide` rejects pending
unmerged migrations; ordinary shared-store startup rejects unapplied known
migrations. LOO-728 cannot land LOO-498 or waive its acceptance.

Subsequent November 4 reconciliation observed #1814 OPEN at
`df3b5e810d3841a19bc5911c332fb42f23193d46`, including the coherent installer
and a revised draft `rex_workspaces` schema. That parent is not integrated here.
The following candidate results remain evidence for `c136addfa`; they do not
validate the newer migration. Parent synchronization, delivery and rebuilding
precede a fresh compatibility check and configured demo.

Rebuilt candidate `0.15.1+c136addfa.dirty`:

- `target/debug/lf install preflight --json`: exit 1, `ahead_pending`, applied
  frontier `wave_removal`, latest known/raised reader floor `rex_workspaces`,
  unmerged `["rex_workspaces"]`; executable compatibility passes 19 references.
  The earlier schema ordering mismatch is resolved; promotion is still rejected.
- `target/debug/lf session list --json`: shared store lacks the unmerged
  `rex_workspaces` migration.
- Installed `lf resume fd5f2333c5f54054b40c4bb5e574a98a --prepare --json`:
  retains that Session ID; preparation launches nothing and selects no candidate
  owner. A development CLI alone does not change the selected installation.
- `git diff --exit-code -- docs/orchestration.md docs/surfaces.md`: pass.

No configured store migration, installation, owner transition or editor change
was performed. A private Home, removed migration, hand-edited ledger, main-only
owner or stale-placement fallback cannot establish the required experience.

## Configured owner and agent failure

Interactive Session: `fd5f2333c5f54054b40c4bb5e574a98a`; owner:
`0756f737-c051-4905-b51b-f24a4b4b6342` (PID 69363); AgentProcess:
`2883597e-d8cd-4586-8ed3-c7c6468d97ad`. Read-only inspection of its transport
returned exactly:

```json
{"answers":1,"seq":0,"kind":"controlled","held":true}
```

The owner has a controller but reports no placement. Candidate `viewed_summary`
correctly leaves placement absent instead of substituting startup environment.
The background caller and Task's primary conversation supply no destination.
Availability alone proves no current viewer placement.

At 21:17 and 21:30 UTC, from this Task checkout with `LF_AGENT_CALLER` intact:

```sh
target/debug/lf open --task LOO-728 --file docs/orchestration.md --file docs/surfaces.md --explain --json
# Rex; both original absolute paths; opening:false
target/debug/lf open --task LOO-728 --file docs/orchestration.md --file docs/surfaces.md --json
# exit 1: Current conversation viewer is unavailable; refusing stale Rex startup placement
```

These attempts used `0.15.1+c136addfa` before the catalog rebuild. No editor
opened; draft preservation, saved correction consumption and conversation
continuity could not be demonstrated. Original docs remained unchanged. Installed
Task status still identified PR #1817 and the same interactive Session.

## Earlier failure and bounded success

The original configured attempt used `0.15.1+13e7d5dac.dirty`, while installed
lf was `0.15.1+ac8e088f5`. Agent presentation failed with the same placement
error. Removing `LF_AGENT_CALLER` diagnostically created two Rex panes, but both
showed `unrecognized subcommand 'tool'`: the installed lf lacked `task tool`.
Those panes exited, so `opening:true` did not establish a working editor.
Missing-file presentation failed without altering docs. Another pre-existing
shell disappeared without a close/input command from the demo; its cause remains
unknown and preservation was not established.

The repair routes placement through the controlling transport viewer and
launches `task_tool::file_editor` directly, avoiding the installed-CLI hop.
An isolated real-Rex probe with private socket, config, state, Home and repository
used a candidate owner, its Rex relay and agent provenance. It opened original
`guide.md` in vi, preserved the same pane/focus/unsaved correction on repeat,
saved into the original file and reread the correction. A missing-file request
left saved content intact. Probe Sessions and process groups were closed; no
production store, provider or existing pane was altered.

Jack Heart later confirmed “ok it opened” in the configured direct development
CLI demo. That command also removed `LF_AGENT_CALLER`, and did not restart the
old owner. Native inventory and screenshots showed the original docs beside the
same conversation; repeat presentation reused the editor. Later missing panes
have no established cause; local swap recovery bytes remain preserved.
[docs-first-dev-demo.md](docs-first-dev-demo.md) retains command context, native
identities, screenshots and feedback. `demo.html` is that historical runbook.
Neither direct CLI success nor the isolated probe establishes configured agent
acceptance or permission to narrow the contract.

## Next owner and required proof

After parent delivery: install a coherent build containing this stack, use the
supported same-Session owner transition, then demo/review the original agent
command. Preserve Session/provider history and drafts. Show original-file
opening, an unsaved correction surviving repeat presentation, saved-edit
consumption by implementation, missing-file recovery and a completing conversation
turn. Jack Heart reserved interactive Rex demo for the primary conversation.
No new routing defect is established by the current diagnosis; repeating an
unchanged candidate/owner through demo cannot supply this proof.

Check: candidate rebuild passed; promotion/startup still fail on the unmerged
parent migration. Context-only compression reuses prior focused checks;
`git diff --check` and `lf context` pass. No background process was started.

Complete pre-compression observations, decisions, native pane IDs and commands,
including unpublished diagnosis, are preserved as Git blob
`3c105e3f2c64cfdd313e0fd6c0303f1971a4e13d` (`git show <blob>`).
