# Review: Discovery Auth Design Doc

## What was implemented

Elaborated the wave item `wave/mobile/02-discovery-auth.md` (64 lines) into a full design doc at `scratch/02-discovery-auth.md` (345 lines). The wave item is deleted — its content is superseded by the design doc.

No code changes. This branch is design-only.

## Key choices

**Single-claim over single-use.** The wave item said "single-use" tokens. The design doc refines this to "single-claim" — a token transitions Available → Claimed on first use and stays valid for reconnects. This prevents WebSocket reconnections from exhausting the pool. Well-reasoned refinement.

**Dropped auto-revoke.** The wave item included "auto-revoke on suspicious patterns (same token from multiple IPs)." The design doc drops this as out of scope. Correct call — pattern detection is complex, tokens already expire in 1 hour, and manual revocation covers the threat.

**Dropped `advertise_url`.** The wave item included this for reverse proxy setups. The design doc drops it. Keeps the scope tight — can be added later if needed.

**DualAuth as a fourth variant, not a mode.** Adding `DualAuth` to the `AuthProvider` enum keeps the type system doing the work. Auth dispatch is a match arm, not a conditional chain. Clean.

## How it fits together

Token lifecycle flows through three systems: lfd mints and validates tokens locally (TokenLedger), studio distributes them to mobile users, and the registration/heartbeat protocol keeps them synchronized. The desktop toggle in Concerto controls whether lfd runs in Static or DualAuth mode.

The design is layered bottom-up: ledger → auth variant → registration → WS re-validation → UI toggle → revocation.

## Risks and bottlenecks

**Studio coordination.** The design assumes studio will implement pool storage and token handout. These endpoints don't exist yet. Implementation can proceed on the lfd side independently, but end-to-end testing requires studio work.

**WS re-validation is architecturally new.** No per-session token check exists today. The 60-second interval timer in the WebSocket select loop is a new pattern that touches a critical path. Will need careful testing.

**Pool exhaustion.** If many mobile users connect rapidly and studio doesn't report `tokens_remaining` fast enough, the pool could empty before replenishment. The design mitigates this with a pool of 5 and threshold of 2, but high-concurrency scenarios aren't analyzed.

## What's not included

- No code — this is the design phase
- Studio-side endpoints (separate repo)
- Auto-revoke on suspicious patterns (dropped from wave item, added to out-of-scope)
- `advertise_url` for reverse proxy (dropped from wave item, added to out-of-scope)
- Actual TLS serving
