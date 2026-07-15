# W2-135 — open questions / hold state

## HOLD at PR1/PR2 boundary (as of 2026-07-15)

**PR1 is landed.** PR #898 (*supervision: BodyObservation state model + projection*)
merged to `origin/main` as `9a0433c7c`, on top of the landed W2-134 delta wire
(`2e4f91cb9`). W2-135 remains open. The runner rotated to the next serial branch
`jack-heart/make-session-bodies-leased-progress-generation-write-lease`
(base `9a0433c7c`).

**Not starting PR2.** The current directive is still **v6**, which explicitly
instructs: land PR1, "leaving W2-135 open and **stopping before PR2
implementation**." No v7 directive has arrived. The loop mechanically rotated the
branch, but an explicit human stop governs over loop momentum. Holding here — no
PR2 design note, no PR2 code — until a directive (or the wave's sequencing
decision) authorizes PR2.

**Why the gate matters beyond the literal instruction:** the wave is actively
sequencing the contract boundary between W2-134's live-turn DTO and W2-135's
broader supervision write-lease. PR2 is "atomic generation/write lease and
process-group ownership" (delivery shape) — precisely the surface the wave said
W2-135 should rebase onto W2-134 for if files overlap. Starting PR2 blind would
pre-empt that supervision.

## What PR2 will be, when authorized (not a design — a pointer)

Per the seed's delivery shape, PR2 = atomic generation/write-lease + process-group
ownership: generation + lease token, process-group reap on supersession, exactly
one writer sampled, stale generations go read-only and exit. Build the ambitious
scratch design (one-screen user model, shared types, state-transition table,
recovery policy, serial PR boundaries) at that point — `lf pr land` cleared the
PR1-phase scratch, so it starts fresh.

## Next wake

Expect a v7 directive authorizing PR2, or a wave sequencing signal. On arrival:
acknowledge, write the PR2 scratch design, then implement the atomic
write-lease + process-group slice on this branch.
