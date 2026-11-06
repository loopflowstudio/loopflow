# LOO-717 / LOO-498 — current boundary

## Follow-up correction — November 5

Jack Heart accepted explicit Desktop Quit release and bounded disconnect cleanup:
the same fifteen-minute absence window begins at detected loss. Reconnect or
another connected Task Session viewer retains; requested work, drafts and pending
requests remain protected. Desktop shells add no hold. This supersedes indefinite
unknown-liveness wording below. LF-owned remote routing carries client heartbeats;
independently opened external SSH relies on its host's loss detection. No external
host registry or LF closure of Rex shells is added. Current repair and proof limits
are in loo-717-shell-lifetimes.md.

## Latest clarification in LOO-717 — November 5

Jack Heart clarified: retain the worktree for an open LF viewer of its Task
Session, not a random shell in that worktree. Jack then identified the viewer's
LF process lifetime as the signal and requested the updated design plus a new
pursue run. Confirmed last viewer exit releases its hold. A native tab remaining
after the LF viewer exits is not itself a hold. This supersedes the stronger
retained-block wording in the handoff below and removes its native-metadata gap.
Other viewers, active requested work and unavailable liveness remain protected.
No LF persistence of external hosts/occurrences and no automatic Rex shell closure.

Current implementation direction is in loo-717-shell-lifetimes.md. Earlier
occurrence/lease/settlement contracts are superseded; history remains at
b81df60579:scratch/rex-coordination.md.

## Complete handoff received before that clarification

# LOO-498 → LOO-717 handoff — November 5, 2026

Jack Heart requested this handoff and asked to open LOO-717 in Rex. This conveys accepted direction and implementation evidence; it does not approve shipping, installation, shared-store migration, or launch of a new implementation Graph.

## Accepted ownership decisions

- LF must not persist external hosts, host workspaces, or host occurrences anywhere: no database rows, sidecars, copy registry, creation journal, or lease-table exception. This is settled, not a question to ask again.
- Config owns defaults/templates for future Task layouts (existing shared/local Wave/repository/global scopes). The existing LF DB owns Desktop sticky Task layouts only.
- External hosts own their arrangements, mutations, ordinary shells and restoration. LF renders its source layout into the native host once, as faithfully as possible. Every explicit lf open launches fresh; returning to previous work happens in the host.
- Repository or Wave maps to a native container (Rex session); Task maps to a tab; workspace means its splits. There is no extra Loop concept.
- LF automatically closes only Loopflow Desktop-owned shells. Do not close Rex shells, impose away timers on them, or require native-shell settlement from Rex.
- A Rex terminal retained for an LF Task Session is enough to protect its checkout from cleanup, even hidden or unfocused. Only confirmed removal of the last relevant terminal can release that protection, subject to other holds and normal cleanup conditions. Unavailable inventory is unknown, never release. A viewer/program exiting does not prove its retained terminal was removed.
- Preserve active requested work, drafts, pending requests, existing provider/Session identities and the recorded Machine/checkout. Opening never launches or replays a provider turn. Explicit input transfer remains separate. No automatic deleted-checkout reconstruction requirement.

## LOO-498 implementation and evidence

PR https://github.com/loopflowstudio/loopflow/pull/1814 ; code pushed through b1864599e76b2eca74a3a03fd8ffc41d287a1682, final notes f129371904.

The unmerged rex_workspaces migration and all HostWorkspace/HostOccurrence/HostCreation persistence were removed. There is no replacement registry. Layout precedence: saved Desktop Task layout, resolved task_layout config, builtin. Setter/config reads reject unusable templates and captured Session/execution state.

Native renderers support Rex, Herdr and cmux. Explicit host overrides; current viewer placement is intended to guide agent-issued opens. cmux resolves the caller workspace's current native group. Tool panes report their Task directory, ordinary shells honor cd; actual app Split is still owed to demo.

One source Rex follower adds started unfinished Tasks across Machines without focus or a new provider. Remote native container handles live only in follower argv/memory, passed from launch receipts; first eligible arrival on another Machine creates its container, later arrivals join it. Closed/seen arrivals stay respected. Uncertain native creation is never blindly retried. Remote failure never falls back locally. Herdr/cmux following is not implemented.

Checks passed: open library 18, config library 77, work_open integration 23, isolated native Rex explicit --host rex regression, fmt/clippy. Remote arrival and cmux group proofs use stand-ins; real two-Machine and cmux app proof remain. Demo still owes native app Split, agent-tool routing after viewer movement, and real-provider input after view again. These are not claims of overall acceptance.

## Exact cleanup integration gap

LOO-717 PR #1811 at inspected b81df60579 exposes lf task presence and stores task_host_occurrences with a 30-second lease and native_settled. That shape conflicts with the decisions above. Its executor observes owner exit; it does not establish retained Rex block removal. Do not wire LOO-498 back into that occurrence table or infer release from owner exit.

An isolated Rex API probe found session.list_blocks returns block_id, label, window_id and owner; block pwd/process provide process identity. No argv/env was exposed. Live LF tools know REX_BLOCK and REX_SESSION. A retained block can be queried by ID, but associating an exited retained block with the Task without LF host records remains unresolved. Labels are editable and not a reliable primary association. Native metadata or another host-owned association may be worth investigating; this is a proposal, not an accepted new registry.

LOO-717 owns the shared cleanup implementation. Reconcile its API with the no-persistence decision; keep cleanup conservative while evidence is unavailable. LOO-498 has not edited or launched LOO-717.

Relevant current source in LOO-498: docs/workspaces.md; scratch/rex-workspace.md; scratch/questions.md; scratch/workspace-demo.md. Full details are included above so a path in another checkout is not the only context.

## Latest configured-path finding

When Jack requested opening LOO-717, a branch lf open --task LOO-717 --host rex attempt failed before creating anything: Current conversation viewer is unavailable; refusing stale Rex startup placement. It returned destination=null, container=null. This means the isolated Rex regression does not prove this configured agent-caller path. Jack then instructed use Rex CLI directly until lf lands. Do not present that failed opening as a successful demo.
