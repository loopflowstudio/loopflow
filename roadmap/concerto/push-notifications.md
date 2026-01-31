---
status: todo
phase: 3
---

# Push Notifications

APNS integration for mobile alerts when waves need attention.

## Current

Local macOS notifications via NotificationService.

## Build

```
┌─────────────┐     ┌─────────────────┐     ┌─────────────┐
│   Your lfd  │────►│    Loopflow     │────►│    APNS     │
│             │     │                 │     │             │
│ "wave needs │     │ looks up your   │     │ pushes to   │
│  attention" │     │ device token    │     │ your phone  │
└─────────────┘     └─────────────────┘     └─────────────┘
```

- Mobile app registers device token with Loopflow
- lfd sends events to Loopflow
- Loopflow pushes to APNS
- Rich payload: wave_id, wave_name, step, reason

## Done when

Mobile gets push notification when wave needs attention, tapping opens that wave.
