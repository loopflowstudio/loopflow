# LOO-593 decisions and remaining questions

Jack Heart selected the November 4 redesign and authorized pursue in
[the working design](mobile-backend.md): Machine manages execution destinations,
server exposes client access, and installation owns registration and updates.
`lf server` now owns the local serving/status commands; installation owns the
hidden client-readiness bridge. Jack authorizes implementation/publication in LOO-593
and a return to interactive demo, superseding the docs-only restriction. The
existing feature graph preserves that demo boundary.
No merge or Auto-Ship approval is given. The working design contains the execution
plan and current PR overlap inventory.

Keep the earlier accepted recovery interaction: automatic exact-owner reconnect,
native status, explicit in-pane Move here, newer control claims preserved and
no input/command replay. Drafts, active turns, pending requests and running Graphs
already protect their owners. The split adds no retention policy.

## Remaining design details

1. Remote command spelling belongs with LOO-718: enabling client access, pairing,
   listing clients and revocation all live under server. The ownership split is
   accepted; retain that branch's existing semantics when mapping its verbs under
   server. This local pursuit supplies the migration handoff without adding remote
   functionality or editing the other checkout.
2. Installation migration must replace old supervisor registration once and
   retain access configuration/credentials without duplicate listeners. Internal
   command aliases are not needed; persisted/installed state does need migration.
3. GUI-less distribution, before-login Mac hosting, removal during active work
   and the phone takeover gesture remain separate product choices.

## Proof still owed by the implementation

- Native rendered Desktop recovery, quit/reopen, reading/copy/Find/scroll and the
  visible Move here action. Headless mailbox tests do not establish these.
- Linux installed replacement; broader Xcode, subscription bounds/cancellation,
  defaults/attribution equivalence and unchanged provider-usage totals.
- Real LOO-718 integration and the composed replacement check after this local
  command/source cutover. The earlier
  signed proof uses the same candidate at a new executable inode, not a released
  version migration. Re-run that composition after implementation changes.

The decisive recovery counterexample remains a lost takeover acknowledgement
followed by a newer viewer's claim. Consume the generation-bound action once;
recover passively, never repeat takeover or uncertain typing.

Historical research is retained under `scratch/reports/`, with the full original
checkpoint at `0bd889e4b860fd3d7c44389a86914317c0d22af6:scratch/`. Shared executors
alone did not prevent lfd orchestration drift; `setsid()` alone does not prove
supervisor isolation; remote proxies cannot borrow Unix peer trust.

Wave memory's existing Machine-service terminology reflects the prior design.
Reconcile it through the next authored memory-maintenance step using this accepted
split; the pursuit's realign step can reconcile it within its authored scope.

## Implementation assumptions (November 4)

Registration migration retains the old bundled plist solely for SMAppService to
inspect/unregister the installed background item. New code never registers it;
it preserves disabled choices before retiring the old label. The private revision-1
socket remains `machine.sock`: changing its address would strand already-running
relays from the previous executable during replacement. Keeping one stable
endpoint/lock is simpler than aliases or a parallel listener. No credentials or
Work records move. This branch has no
persisted remote settings; LOO-718 must retain its credential/config locations
when integrating the namespace change.
