# Mobile

Remote view of loopflow from iOS. Check on waves, browse roadmap, chat with agents — without opening a laptop.

## Vision

The phone is a read surface first. Conductors check what's happening across waves while walking, in meetings, between things. Mobile chat comes second — start a conversation from the phone, and later share that session across devices. Build work stays on the laptop.

### Not here

- Running build work from the phone
- Editing code, rebasing, landing
- Anything that requires a full keyboard

## Tasks

1. **`remote-lfd-connection`** (p1) — auth to a remote lfd host, TLS, reconnect. Demo: scan QR, app syncs
2. **`see-your-waves`** (p1) — wave list with live status. Demo: morning train, know what's happening
3. **`see-wave-tasks`** (p1) — drill into a wave's roadmap. Demo: check what's on the plate for a wave
4. **`start-a-chat-on-mobile`** (p2) — start a new agent session from the phone. Demo: design thought on a walk, agent responds
5. **`shared-chat-with-desktop`** (p3) — sessions cross devices. Demo: start at desk, continue on phone, finish at desk

## Risks

- TestFlight / distribution overhead — the app has to actually ship to be useful
- Auth for remote lfd — tokens, provisioning, discovery all need a clean story
- Scope creep — "just one more thing" pulls toward build work. Line: if it requires a keyboard, it's not mobile
