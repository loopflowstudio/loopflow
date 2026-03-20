---
linear_id: 336b92b0-82cc-4a82-9a2e-dde63f5042c7
---
# 02: Phone Deploy

**Finish line:** Concerto iOS connects to a remote lfd instance and provides useful wave monitoring and intervention on the go.

## What to validate

1. **Remote connection from iPhone.** Concerto connects to lfd on Mac Mini (or any remote host). WebSocket streaming works over cellular and wifi. Reconnection after network transitions.

2. **Block queue on phone.** Blocks surface on iOS. Human can review and make decisions (approve, reject, defer) from the phone. Decisions flow back to the system.

3. **Wave monitoring.** See which waves are running, recent activity, any blocks. Portfolio view works at phone scale.

## Done when

- Concerto iOS connects to remote lfd and stays connected
- Block queue renders and accepts decisions on iPhone
- Wave status visible without opening a laptop
