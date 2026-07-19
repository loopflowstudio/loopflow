# Slice 2 repair: promotion must not impersonate the User

## Problem

Project promotion stores typed parent/child Wave ancestry, starts the child
listener, then `ops/project.rs::wake_child` shells out to plain `lf chat` with
generated prose. The Wave journal records that machine-authored bootstrap as an
unattributed `UserMessage`. Radio and bylines are gone, but machine speech still
masquerades through the human-only Chat surface.

## Required behavior

- Promotion records parentage in the typed Wave registry and starts child
  residency as it does now.
- The promoted child begins its first Wave pass promptly after the listener is
  live.
- That wake is typed or derived from durable promotion state. It does not call
  `lf chat`, POST a human message, append `UserMessage`, invent a byline, or
  restore Radio/channel identity.
- The durable parent relation remains truth. Any process signal is only a wake
  optimization and may be retried without duplicating a promotion fact or
  starting overlapping passes.
- Keep this repair inside the current Wave runtime. Do not pre-choose the
  generic Home/Work server topology or add a second lifecycle model.
- Add no compatibility command, alias, generic Message aggregate, approval, or
  Session identity.

Choose the smallest honest implementation after tracing listener startup,
resident inbox replay, and heartbeat scheduling. Prefer an existing typed
runtime/evidence seam. If none exists, add only the promotion-specific wake
needed to preserve immediate behavior; do not create a general event bus.

## Done when

- [ ] `complete_promotion` cannot invoke `lf chat` or the human `/messages`
      door.
- [ ] A deterministic behavior test proves a typed promotion wake starts one
      child pass and does not append a `UserMessage`.
- [ ] Repeating or replaying the wake cannot create duplicate durable promotion
      facts or overlapping child passes.
- [ ] The human thread still rejects machine-only fields/ops and accepts only
      real User message, steer, and interrupt operations.
- [ ] The current schema test explicitly proves `bus_messages` and
      `bus_cursors` are absent after migration.
- [ ] The CLI parser rejects `lf radio`; no hidden compatibility spelling
      remains.
- [ ] Stale byline wording is removed from Wave journal tests/docs.
- [ ] Focused Wave, promotion, parser, migration, fmt, and Clippy proofs pass.
- [ ] `scratch/feedback-runtime-review.md` records the repair and replaces its
      inaccurate typed-promotion claim with exact behavior.
