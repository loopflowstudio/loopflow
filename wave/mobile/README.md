# Mobile

Remote read surface for loopflow from iOS.

## Vision

The phone is for checking on work, not doing build work. Open the app, connect to a remote lfd, see your waves, inspect the roadmap, and understand what needs attention without opening a laptop.

This wave stays view-only on purpose. Chat can come later once the read path is solid.

## Priorities

1. **`remote-lfd-connection`** (p1) — connect to a remote host cleanly and stay connected
2. **`see-your-waves`** (p1) — list waves with live status and obvious health
3. **`see-wave-tasks`** (p1) — drill into a wave's roadmap and read full item context

## Mobile owns

- Remote connection, auth, and reconnect behavior for the iOS client
- Read-only wave and roadmap browsing away from the laptop
- The smallest self-contained remote surface that proves the phone has a role

## Not here

- Running build work from the phone
- Editing code, rebasing, landing, or any keyboard-heavy workflow
- Native chat sessions for this phase

## Risks

- Remote auth and host discovery need to feel simple or the whole surface collapses
- Read-only scope will get pressure from “just let me do one more thing”; hold the line
- Mobile only earns its place if reconnect, caching, and empty states feel calm under bad network conditions
