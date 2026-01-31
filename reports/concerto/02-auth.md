# Auth Model

Simple split:

| Scenario | Auth |
|----------|------|
| **Local** (lfd + Concerto on same Mac) | None needed |
| **Remote** (mobile, or Concerto from another machine) | Loopflow account |

## Local

Everything on one machine. No auth, no internet required. Works today.

## Remote

Want mobile access? Sign up for Loopflow (GitHub OAuth). Your lfd registers with Loopflow, receives tokens, validates mobile connections.

```
┌─────────────┐      ┌─────────────────┐      ┌─────────────┐
│   Mobile    │─────►│    Loopflow     │◄─────│   Your Mac  │
│  Concerto   │ auth │   (identity)    │ reg  │     lfd     │
└─────────────┘      └─────────────────┘      └─────────────┘
                            │
                            │ validates
                            ▼
                     Mobile connects to
                     your lfd with token
```

No middle states. No "self-hosted but also remote with local API keys." Local = offline, remote = Loopflow account.

## Why This Split

- Internet dependency for an internet feature is fine
- Forcing Loopflow registration for remote access is acceptable
- Keeps security model simple and clear
- Same auth infrastructure enables push notifications
