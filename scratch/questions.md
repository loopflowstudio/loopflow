# LOO-728 — integration assumptions

November 4, 2026: `lf sync --plan origin/jack-heart/write-docs-first` refuses
an explicit target for a stacked Task because sync selects the stack parent.
Jack Heart explicitly requested merging this source while retaining LOO-498 as
parent. A local Git merge of the pinned source prepares the integration without
changing the Task's parent; Loopflow will finalize the merge. No source branch,
checkout or PR is removed.

The following inherited LOO-498 notes are historical dependency context, not
work assigned to this Task. Their full source is
`da41d2031f:scratch/questions.md`; the parent plan and demo remain in the
adjacent notes. LOO-728's active plan is `write-docs-first.md`.

## LOO-498 — unresolved restoration and host evidence

**Demo correction, November 3:** [workspace-demo.md](workspace-demo.md) records
that the configured LF opening failed at database-schema compatibility. The
raw native-split API probe does not prove the app's Split button omits cwd;
that UI path remains unverified. Do not require a scope reduction on the raw
API evidence alone. A separate native split-demo session is available for review.

Jack Heart requested documentation first. The primary conversation drafted
[the user guide](../docs/workspaces.md) before further feature implementation or
publication. It describes the agreed experience and explicitly marks current
delivery gaps; use it to reconcile the product contract. Do not silently weaken
native split, restore, agent routing or lifecycle requirements to match the branch.

Jack Heart's November 3 build approval supersedes the mistaken-launch stop.
The current scope, implementation and remaining work are in `rex-workspace.md`;
no separate Task-view scope, Auto-Ship or merge approval was added.

Two product choices remain only if supported host evidence cannot resolve them:

- After Rex restores a block as a new login shell, how should explicit restoration
  recover familiar tools without replacing subsequent native work: an explicit
  replacement/new occurrence, or an accepted narrower restore contract? Neither
  is accepted. Stable block IDs do not establish command continuity.
- What Rex release policy should express leaving a Task if public APIs cannot
  observe use/away reliably? Attached clients and active-window inventory are
  insufficient; unknown cannot start destructive retirement. The accepted
  fifteen-minute completed-and-away policy remains, not indefinite retention.

**Native split capability review, November 3.** Jack Heart's native-split
requirement is contradicted by the installed Rex revision-8 API: new native
shells anchored to two different Task directories in one session both start in
the isolated HOME. `scratch/rex-host-probe.py` reproduces this through
`session.new_split` and each new shell writing `pwd -P`. Public creator options offer
per-block cwd; the method inventory offers no per-Task/window default setter.
Account-wide Lua configuration is outside the accepted design. Dependent work
stops at this capability boundary pending a supported host change or Jack's
design judgment; no narrower acceptance is assumed. The independent passive
arrival fix now starts its Task viewer in the validated Task checkout.

The same isolated probe confirms native children on a custom server have
REX_SESSION/REX_BLOCK but no REX_SERVER. Current-viewer placement remains the
right path for recorded viewers; unknown custom endpoints cannot be guessed from
opaque block IDs. No running-app/socket scan or global host configuration was added.

Reconciliation assumption, November 3: LOO-717's published implementation and revised
coordination comment `30bf4970-3bb1-481f-8200-83a6734cb26e` establish the intended
integration API, not delivered integration in LOO-498. The inspected source now
requires native settlement separately from away/closed policy. No new cleanup
owner fills the gap; materialization observations are not lifetime holds.
PR #1811 is open at demo; publication does not establish lifecycle acceptance.
Non-default-server bare open and remote following remain engineering work under
accepted scope. Arrival activation adds tools once beside the original passive
pane; follower recovery preserves old native tabs and uses an OS lock for process
liveness. These are implementation choices, not new restoration or retention policy.
