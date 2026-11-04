# LOO-593 decisions and remaining questions

Jack Heart selected the November 4 redesign and authorized pursue in
[the working design](mobile-backend.md): Machine manages execution destinations,
server exposes client access, and installation owns registration and updates.
`lf server` now owns the local serving/status commands; installation owns the
hidden client-readiness bridge. Jack authorizes implementation/publication in LOO-593
and a return to interactive demo, superseding the docs-only restriction. The
existing feature graph preserves that demo boundary.
No merge or Auto-Ship approval is given. The working design contains remaining
acceptance and the dated PR overlap inventory.

The earlier accepted recovery interaction remains: automatic exact-owner reconnect,
native status, explicit in-pane Move here, newer control claims preserved and
no input/command replay. Drafts, active turns, pending requests and running Graphs
already protect their owners. The split adds no retention policy.

## Remaining design details

1. Remote command spelling belongs with LOO-718: enabling client access, pairing,
   listing clients and revocation all live under server. The ownership split is
   accepted; retain that branch's existing semantics when mapping its verbs under
   server. This local pursuit supplies the migration handoff without adding remote
   functionality or editing the other checkout.
2. Local supervisor migration is implemented with one stable socket/lock and
   disabled choices preserved. The renamed composed replacement proof passed during
   publication review; credential/config preservation belongs to LOO-718 integration.
3. GUI-less distribution, before-login Mac hosting, removal during active work
   and the phone takeover gesture remain separate product choices.

## Proof still owed by the implementation

- Native rendered Desktop recovery, quit/reopen, reading/copy/Find/scroll and the
  visible Move here action. Headless mailbox tests do not establish these.
- Linux installed replacement; broader Xcode, subscription saturation/cancellation
  cleanup, defaults/attribution beyond tested parity and unchanged provider-usage totals.
- Real LOO-718 integration. The composed replacement check passed after this local
  command/source cutover. The earlier
  signed proof uses the same candidate at a new executable inode, not a released
  version migration. The namespace-cutover rerun passed during publication review.

The decisive recovery counterexample remains a lost takeover acknowledgement
followed by a newer viewer's claim. Consume the generation-bound action once;
recover passively, never repeat takeover or uncertain typing.

Historical research is retained under `scratch/reports/`, with the full original
checkpoint at `0bd889e4b860fd3d7c44389a86914317c0d22af6:scratch/`. Shared executors
alone did not prevent lfd orchestration drift; `setsid()` alone does not prove
supervisor isolation; remote proxies cannot borrow Unix peer trust.

The November 4 graph realignment updates Product memory to the accepted split.
No child Wave directories or memories exist under `.lf/wave/product/` in this checkout.

## Implementation assumptions (November 4)

Failed first setup consumes readiness for the client lifetime. Work refresh uses
the existing observational status command to discover the endpoint; no Swift
path resolver is introduced. Inspection also found the first successful attach
after failed startup still carried replacing intent. It now uses existing passive
recovery, preserving claims acquired during the outage. These corrections apply
Jack Heart's accepted reconnect-only rule without changing installation policy.

Registration migration retains the old bundled plist solely for SMAppService to
inspect/unregister the installed background item. New code never registers it;
it preserves disabled choices before retiring the old label. The private revision-1
socket remains `machine.sock`: changing its address would strand already-running
relays from the previous executable during replacement. Keeping one stable
endpoint/lock is simpler than aliases or a parallel listener. No credentials or
Work records move. This branch has no
persisted remote settings; LOO-718 must retain its credential/config locations
when integrating the namespace change.
