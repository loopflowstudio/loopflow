---
linear_id: e26a973c-fa10-446a-8de7-74e1685d1e96
---
# 03: Team Workflow

**Finish line:** Two people can connect to the same lfd instance and collaborate on waves without stepping on each other.

## What to validate

1. **Multi-user auth.** Team auth mode (WorkOS OAuth) issues distinct JWTs. Each user sees their own session. Permissions don't leak.

2. **Shared waves.** Both users see the same wave state. One creates a wave, the other can run it. Block queue shows blocks relevant to whoever is looking.

3. **Concurrent sessions.** Two active sessions on different waves don't interfere. WebSocket broadcasts go to the right clients.

## Done when

- Two users authenticate independently against the same lfd
- Wave state is shared and consistent
- No session leakage or interference under concurrent use
