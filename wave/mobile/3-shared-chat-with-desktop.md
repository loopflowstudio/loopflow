---
asana_id: '1214269990746869'
---
# Shared chat with desktop

**Finish line:** A session started on desktop can be picked up on mobile mid-conversation and vice versa. The session is the unit — both devices are clients; lfd owns state.

**Needs:** `workflows/chat-session-api`, `mobile/start-a-chat-on-mobile`, `desktop/native-chat-ux`

## Context

After mobile can start chats and desktop has rich chat UX, this glue makes them one surface across devices. Relies on the resumable stream + bidi input from `workflows/chat-session-api`.

## Daily experience

At your desk, ask Claude to design a feature. Streaming response starts. Leave for a meeting; on the way, open phone, same session is there with the full reply. Send a follow-up from phone. Back at desk, the follow-up and its reply are in desktop chat. No re-context, no copy-paste, no version drift.

## Done when

- Sessions are visible from both clients with full transcript
- Mid-turn input from one device interrupts + steers for both
- No event loss on device switch
- Session list on mobile shows recent cross-device sessions
