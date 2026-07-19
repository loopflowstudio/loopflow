# Make promotion wake retryable

## Problem

The typed `PromotionWake` is currently delivered only by a best-effort HTTP
nudge. Ordinary `StoreObserver` polling deliberately refuses to infer a new
promotion from `parent_wave_id`, because ancestry may predate this promotion.
If the nudge is lost, the promotion occurrence disappears permanently.

## Design

Store the occurrence separately from ancestry:

```rust
struct Wave {
    parent_wave_id: Option<WaveId>,
    promoted_at: Option<OffsetDateTime>,
}
```

Ordinary chord ancestry and `with_parent` leave `promoted_at == None`.
`complete_promotion` atomically persists the parent link and a first-write
promotion timestamp before residency or HTTP delivery. Replays preserve that
timestamp.

`StoreObserver::poll_once` may then derive the typed wake only when both the
parent link and promotion occurrence exist. Runtime/journal identity keeps the
wake exactly once across polling and listener restart. The HTTP request remains
only a latency optimization; failure is honest and heartbeat polling retries
from durable state.

Use migration `0.12.003`. Existing parent links migrate with
`promoted_at == None`, so old ancestry never manufactures a new wake.

## Done when

- A Wave with only `parent_wave_id` never emits `PromotionObserved`.
- `complete_promotion` persists one `promoted_at` occurrence before trying the
  listener and preserves it on replay.
- A failed or absent HTTP nudge is recovered by the next observer poll and
  queues the same typed `PromotionWake` once.
- Repeated polls and listener close/reopen do not duplicate a consumed wake.
- Parent identity is still verified from registry truth; request strings are
  not authority.
- Wave row reads/writes, migrations, tests, and docs include the new fact with
  no compatibility reader or second outbox abstraction.
- Focused registry/runtime/promotion, migration, format, and Clippy proofs pass.

