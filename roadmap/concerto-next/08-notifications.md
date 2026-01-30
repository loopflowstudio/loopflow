# Notifications

## Architecture

Push notifications go through Loopflow's account system (same infra as auth).

```
┌─────────────┐     ┌─────────────────┐     ┌─────────────┐
│   Your lfd  │────►│    Loopflow     │────►│    APNS     │
│             │     │                 │     │             │
│ "wave needs │     │ looks up your   │     │ pushes to   │
│  attention" │     │ device token    │     │ your phone  │
└─────────────┘     └─────────────────┘     └─────────────┘
```

## Registration Flow

1. Mobile Concerto gets APNS device token from iOS
2. Sends token to Loopflow (with your account)
3. Loopflow stores: "jack's phone is token xyz"
4. lfd tells Loopflow: "wave needs attention"
5. Loopflow pushes to APNS with your token
6. Phone wakes up, shows notification

## Payload

Rich payload with enough data to be useful without opening app:

```json
{
  "alert": "feature-auth waiting: design",
  "wave_id": "abc123",
  "wave_name": "feature-auth",
  "step": "design",
  "reason": "interactive"
}
```

## Philosophy

**Infra first, UX later.**

For now:
- Pipe works: lfd → Loopflow → APNS → phone
- Rich payload with enough data
- Basic on/off

Later:
- Quiet hours
- Batching / digest mode
- Fine-grained filters (only failures, only interactive, etc.)
- Notification actions (approve/reject from notification)

No Loopflow account = local only = no push notifications (you're at your Mac anyway).

Loopflow account = remote access + push. Same registration enables both features.
