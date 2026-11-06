# LOO-717 / LOO-498 — ownership boundary

Jack Heart's November 5–6 decisions are authoritative in
[the shell lifetime contract](loo-717-shell-lifetimes.md): only LF viewers of
Task Sessions retain a checkout, including passive, hidden and unfocused viewers.
Confirmed viewer exit releases its hold; detected disconnect starts the same
fifteen-minute absence window. Reconnect cancels cleanup. Requested work, drafts
and pending requests remain protected. A surviving native tab or ordinary shell
adds no hold. LF closes only Desktop-owned shells, never Rex's.

LF stores no external host, workspace or occurrence identities: no registry,
sidecar, creation journal or lease exception. Host arrangements, mutations,
ordinary shells and restoration stay native. Config supplies future layouts;
the existing LF database owns Desktop sticky Task layouts only. Explicit host
opens are fresh; they preserve Session/provider identity, recorded Machine and
checkout, and never replay a turn. Input control is separate from retention.

LOO-498 owns native layout creation; LOO-717 owns shared cleanup. The shared viewer
protocol now crosses LF-owned SSH routes. External SSH connections use their
host's own loss detection. No native inventory or shell-settlement handshake
is required for checkout cleanup, and deleted checkouts are not reconstructed.

## Handoff evidence and remaining proof

The full November 5 handoff, superseded retained-native-tab interpretation and
native metadata probe are preserved at
`a1067c99cc:scratch/rex-coordination.md`. The earlier occurrence design is at
`b81df60579:scratch/rex-coordination.md`; neither is current direction.

LOO-498 reported PR #1814 through `b1864599e76b2eca74a3a03fd8ffc41d287a1682`
(notes `f129371904`): removed external persistence; Rex/Herdr/cmux rendering;
one Rex follower with remote handles only in argv/memory. Herdr/cmux following
was not implemented. Reported focused checks used stand-ins for remote arrivals
and cmux grouping, leaving native Split, viewer-movement routing, real-provider
input and two-Machine behavior to demo. These results do not establish acceptance.

The configured `lf open --task LOO-717 --host rex` attempt failed before creation
with “Current conversation viewer is unavailable; refusing stale Rex startup
placement” (`destination=null`, `container=null`). Jack then directed use of the
Rex CLI until lf lands. The isolated Rex regression does not prove that caller
path. LOO-498 neither edited nor launched this Task. Handoff grants no shipping,
installation, live-store migration or competing implementation authority.
