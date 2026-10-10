# LOO-447 assumptions

2026-10-10: move the watchdog outside the provider process group, retaining its
captured target PGID and pre-exec readiness handshake. Otherwise the watchdog
counts itself as a surviving helper while waiting for the holder's EOF, so
natural group exit cannot release custody. Holders still require confirmed whole
group death; leader death and unknown inventory never release surviving helpers.
This revises the draft's watchdog placement, not its preservation requirement.

2026-10-10: OpenCode v1.2.0's PATCH Session route accepts title/time, not
permissions. Remove the unsupported permission writer and its permissive fixtures.
Creation carries the original rules and a per-AgentProcess title saved in the
existing attempt. After a lost response, exact-title native listing recovers one
identity; absent/ambiguous evidence remains uncertain without replay. A title
changed externally before recovery cannot establish identity. Existing native
conversations retain their rules. This replaces the draft's separate setup phase;
The correction and process-death creation readback are implemented at
`07529ac3d`; public transport is now the remaining implementation, not blocked
on this protocol correction.
