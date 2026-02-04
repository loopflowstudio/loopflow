---
status: todo
phase: 2
---

# Loopflow Auth

GitHub OAuth for remote access. Local = no auth, remote = Loopflow account.

## Current

Everything local, no auth needed.

## Build

```
┌─────────────┐      ┌─────────────────┐      ┌─────────────┐
│   Mobile    │─────►│    Loopflow     │◄─────│   Your Mac  │
│  Concerto   │ auth │   (identity)    │ reg  │     lfd     │
└─────────────┘      └─────────────────┘      └─────────────┘
```

- GitHub OAuth sign-in flow
- Token storage in Keychain
- Token refresh handling

## Done when

User can sign in with GitHub, receive tokens for remote access.
