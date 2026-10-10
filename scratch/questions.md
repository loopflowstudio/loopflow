# LOO-447 assumptions

2026-10-10: move the watchdog outside the provider process group, retaining its
captured target PGID and pre-exec readiness handshake. Otherwise the watchdog
counts itself as a surviving helper while waiting for the holder's EOF, so
natural group exit cannot release custody. Holders still require confirmed whole
group death; leader death and unknown inventory never release surviving helpers.
This revises the draft's watchdog placement, not its preservation requirement.
