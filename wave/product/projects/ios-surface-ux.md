# iOS Surface UX

The iOS app is the mobile conductor surface: lightweight awareness, steering,
approval, interruption, and recovery when the user is away from the Mac.

## KRs

- Five real away-from-Mac days are steered entirely from iOS — decisions
  made, work interrupted and resumed — without breaking the steward thread
  once.
- A week of notifications wakes the user only for real decisions: every
  notification is rated decision/noise after the fact, and noise trends to
  zero.
- Wave attention state on iOS matches the Mac and CLI view continuously —
  divergence is a failure event.
- iOS exposes the shared loopflow API without inventing mobile-only
  concepts, holding across a month of API evolution.
