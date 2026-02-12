---
status: todo
phase: 2
---

# Push Notifications

APNS integration for mobile alerts when waves need attention.

## Current

Local macOS notifications via NotificationService.

## Build

```
┌─────────────┐     ┌─────────────────┐     ┌─────────────┐
│   Your lfd  │────►│    Loopflow     │────►│    APNS     │
│             │     │     Studio      │     │             │
│ "wave needs │     │ looks up your   │     │ pushes to   │
│  attention" │     │ device token    │     │ your phone  │
└─────────────┘     └─────────────────┘     └─────────────┘
```

- Mobile app registers device token with loopflow.studio
- lfd sends events to loopflow.studio when wave needs attention
- loopflow.studio pushes to APNS
- Rich payload: wave_id, wave_name, step, reason

## Notification triggers

- Wave waiting for input (awareness—interactive action requires laptop)
- Wave completed
- Wave errored
- PR ready for review
- PR merged

## Done when

Mobile gets push notification when wave needs attention, tapping opens that wave in app.
