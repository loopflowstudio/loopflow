# loopflow.studio: Auth v0

Minimal auth service for mobile clients. Lives in `../studio` repo.

---

## Overview

loopflow.studio provides:
1. Auth (JWT issuance)
2. Discovery (where is your lfd?)

loopflow.studio does NOT provide:
- Event relay
- Webhook proxy
- Traffic relay

---

## Auth Flow

```
┌──────────────┐     ┌─────────────────┐     ┌──────────────┐
│  Mobile App  │────►│ loopflow.studio │────►│ OAuth Provider│
│              │     │                 │     │ (GitHub/etc) │
└──────────────┘     └─────────────────┘     └──────────────┘
       │                     │
       │◄────────────────────┘
       │         JWT
```

1. Mobile app opens loopflow.studio auth URL (ASWebAuthenticationSession)
2. User authenticates with GitHub/Google/Apple
3. loopflow.studio issues JWT
4. Mobile app receives JWT via callback URL
5. Mobile app stores JWT in Keychain

**OAuth providers:**
- GitHub
- Google
- Apple

**JWT claims:**
```json
{
  "sub": "user_123",
  "email": "user@example.com",
  "iat": 1234567890,
  "exp": 1234567890
}
```

**Token refresh:** Issue long-lived tokens for v0. Add refresh flow later if needed.

---

## Discovery

Mobile app asks loopflow.studio where the user's lfd is.

**Endpoint:**
```
GET /api/v1/daemons/discover
Authorization: Bearer <JWT>
```

**Response:**
```json
{
  "lfd_url": "https://100.x.x.x:2486",
  "registered_at": "2024-01-15T10:30:00Z",
  "last_heartbeat": "2024-01-15T12:45:00Z"
}
```

**If no lfd registered:**
```json
{
  "lfd_url": null,
  "message": "No lfd registered. Start lfd on your machine or use managed hosting."
}
```

**If lfd stale (no heartbeat in 5 minutes):**
```json
{
  "lfd_url": "https://100.x.x.x:2486",
  "stale": true,
  "last_heartbeat": "2024-01-15T10:30:00Z"
}
```

---

## lfd Registration

lfd registers with loopflow.studio on startup. Already partially implemented in `registration.rs`.

**Endpoint:**
```
POST /api/v1/daemons/register
Authorization: Bearer <JWT>
```

**Body:**
```json
{
  "url": "https://100.x.x.x:2486",
  "version": "0.7.2"
}
```

**Heartbeat:**
```
POST /api/v1/daemons/heartbeat
Authorization: Bearer <JWT>
```

Every 60 seconds while lfd is running.

---

## Scope

**In scope:**
- OAuth flow (GitHub, Google, Apple)
- JWT issuance
- Discovery endpoint
- lfd registration endpoint
- Heartbeat endpoint

**Out of scope:**
- Managed hosting (future)
- GitHub webhook handling (managed-only, future)
- Event relay
- User management UI
- Billing
