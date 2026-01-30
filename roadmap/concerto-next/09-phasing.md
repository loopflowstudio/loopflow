# Phasing

## Phase 1: Conduct + Improvise on macOS

- Dashboard with wave status groupings
- Connect flow with "Continue" button
- Improvise: area picker, step runner
- Land flow
- Local notifications
- All local, no auth

## Phase 2: Remote access foundation

- Loopflow accounts (GitHub OAuth)
- lfd registration with Loopflow
- gRPC terminal streaming
- WaveService protocol abstraction

## Phase 3: Mobile (iOS/iPad)

- Same Conduct + Improvise UI
- Remote terminal view
- Push notifications (APNS)

## Phase 4: Rust lfd

- gRPC implementation
- Same protocol, new backend

## Out of Scope (for now)

**Loopflow-hosted lfd**: Comes after container isolation (Stage 6), Linux execution. For now, lfd runs on your Mac, mobile connects remotely.

**Cross-repo work**: Window per repo today. Cross-repo is interesting but later.

---

## Done When

```bash
# Conduct works
# - Wave dashboard shows status correctly
# - "Connect" opens interactive terminal session
# - "Continue" button advances to next step
# - "Land" submits to merge queue
# - Notifications fire for waiting waves

# Improvise works
# - Can create wave with area
# - Can run individual steps on wave
# - Can add stimulus to transition to Conduct

# Protocol abstraction works
# - Same UI works against Python lfd (now) and Rust lfd (future)

# Mobile works (Phase 3)
# - Remote terminal streams from lfd
# - Same Conduct/Improvise experience
# - Push notifications
```
