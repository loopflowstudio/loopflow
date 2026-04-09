# Concerto Mobile

**Finish line:** Concerto ships on iOS. Pair with a QR code, monitor waves, drive decisions, read agent output — from your phone.

## Context

LoopflowCore (13,168 lines) already targets iOS 18+. Models, networking, state management, auth, event service, and 11 cross-platform SwiftUI views are shareable. This item builds ConcertoMobile as a new target consuming that shared core.

## Architecture

Separate iOS target, not platform conditionals in the existing macOS app.

```
LoopflowCore (shared)     ← already iOS-ready
├── Concerto (macOS)      ← untouched
└── ConcertoMobile (iOS)  ← new
```

ConcertoMobile links LoopflowCore and builds phone-optimized UI. No AppKit, no GhosttyKit, no tmux, no subprocess management.

## Screens

**Four core screens, tab-based navigation:**

### 1. Wave List (home)
- All waves with status badges (running, idle, stuck, completed)
- Current step name, time in step
- Attention count badge per wave
- Pull to refresh, real-time updates via WebSocket
- Tap → Wave Detail

### 2. Wave Detail
- Current step, flow progress
- Streaming output (styled text, ANSI colors, no full terminal)
- PR link (tap to open in Safari)
- Start/stop controls
- Recent run history
- If activity normalization has landed: structured activity feed (file edits, commands) instead of raw output

### 3. Attention Queue
- All unresolved attention items across waves
- Full context per item (what the agent is asking, relevant code/diff)
- Action buttons: approve, deny, skip, defer
- Keyboard-accessible for iPad with hardware keyboard
- Card-based layout — focus on one item at a time, swipe or tap to proceed

### 4. Settings
- Paired daemons list (name, status, last seen)
- "Pair New Device" → camera QR scanner
- Manual paste of pairing link as alternative
- Token management (revoke individual pairings)
- Connection status indicator

## Design constraint

The app must support the full driving spectrum — not just passive monitoring. A user actively driving a looping wave (reviewing designs, sending feedback, watching implementation) is a first-class use case. The session/output view and attention queue are as important as the wave list.

## Terminal on mobile

Ship with rendered text output first. `OutputLine` events rendered as styled text in a ScrollView. Reuse `LiveOutput.swift` from the shared views. ANSI color parsing for readability.

Full terminal (xterm.js in WKWebView or SwiftTerm) is a follow-up based on user demand.

## Connection model

Phone connects through the Studio relay (see `2-studio-relay.md`). LoopflowCore's existing `LocalWaveService` and `LocalEventService` work unchanged — the relay is transparent. The only new networking code is:

- QR scanner that extracts relay URL + daemon_id + token
- Store pairing credentials in iOS Keychain
- Token refresh handler

Connection state UI: connected (green dot), reconnecting (yellow), disconnected (red) in the tab bar or status area.

## What's NOT in scope

- Push notifications (APNs) — revisit after the app is in hands
- Full interactive terminal
- Diff viewer
- Wave creation or configuration
- Voice input

## Done when

- ConcertoMobile builds and runs on iPhone (iOS 18+)
- QR pairing connects to a remote lfd through Studio relay
- Wave list shows real-time status
- Wave detail streams agent output
- Attention items can be approved/denied from the phone
- App handles wifi→cellular transitions gracefully (reconnect within seconds)
- TestFlight distribution to at least one device
