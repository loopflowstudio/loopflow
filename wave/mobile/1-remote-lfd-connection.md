# Remote lfd connection

**Finish line:** The iOS app authenticates to a remote lfd host, remembers it, and reads live state from it. Single setup flow (URL + token, QR, or OAuth), TLS by default, reconnect cleanly after network changes.

## Context

Until now lfd has been local-only in daily use. For a read surface on phone, the phone needs to talk to an lfd running somewhere (Mac Mini at home, cloud VM, etc). Auth, discovery, and resilience are the gate.

## Daily experience

First time: install app, scan a QR from laptop (or paste URL + token), app connects. Every day after: open app, it's already connected, it syncs. Network hiccup on subway: app reconnects when coverage returns without losing local state.

## Done when

- Single-screen setup flow works end-to-end
- Token storage uses Keychain
- TLS by default
- App recovers from network drops and device sleep without manual reconnect
- Clear error states when token expires or host is unreachable
