---
status: todo
phase: 2
---

# lfd Registration

lfd registers with Loopflow service to enable remote connections.

## Current

lfd runs locally only, no registration.

## Build

- lfd calls Loopflow API on startup with machine identifier
- Receives connection tokens
- Validates incoming mobile connections against Loopflow
- Heartbeat to maintain registration

## Done when

lfd can register with Loopflow and validate remote client connections.
