# W2-133 — `lf status` delivers its promised audit snapshot

## The two failures (dogfood, 2026-07-14)

1. `lf status product --json` emits `{wave, loop_state, projects}` only. Its own
   help promises "Project/Task hierarchy, **runs**, **attention**, and live loop
   state". Both promised fields are absent from the wire shape entirely.
2. Bare `lf status --json` inside a resident wave fails: `wave <UUID> is not in
   the registry`. `ambient_wave()` reads `LF_WAVE_ID` (a UUID) and hands it to
   `get_wave_by_name`. The ambient default has never worked from inside a wave —
   the one place it exists to serve.

## The snapshot contract

`WaveDetailSnapshot` gains two fields. Both are *evidence*, not data: the
absence of a reading and the reading "nothing happened" are different facts and
the wire says which.

```rust
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Evidence<T> {
    Ok { items: Vec<T>, truncated: bool },
    Unavailable { reason: String },
}
```

- `{"state":"ok","items":[],"truncated":false}` — we read the ledger; this wave
  has no runs in the window. A real, checkable claim.
- `{"state":"unavailable","reason":"run ledger unavailable: …"}` — we could not
  read. Never rendered as emptiness, never omitted.
- `truncated` names the cap out loud, so a 50-run window can't read as "that's
  all there was".

### `runs: Evidence<RunLedgerEntry>`

Folded from the same `run_events` ledger `lf runs` reads, through the same
`summarize()` — one implementation, no second fold. Filtered to this wave by the
ledger's `wave` column, 7-day window, newest 50, active (`running`) runs never
dropped by the cap. `RunLedgerEntry` becomes `pub`; it was already the `lf runs
--json` wire type.

### `attention: Evidence<AttentionItem>`

Derived from the durable Session registry — not invented, not a new store.

```rust
struct AttentionItem {
    kind: AttentionKind,     // project | task
    id: String,              // session id
    subject: String,         // project slug / task identifier
    owner: NextMoveOwner,    // who must move (reuses the existing enum)
    reason: String,          // the session's own status_reason, or the audit finding
    since: String,           // RFC3339 status_at
    age_secs: i64,           // age of that state, now - status_at
}
```

Two rules, both truthful over registry state:

1. **Someone else owns the next move.** A Session whose `next_move.owner` is not
   itself (`Wave`, `Review`, `Human`, `Ci`, `External`) is waiting on somebody.
   Reason is the session's own `status_reason` — its recorded reason, not a
   guess.
2. **The process is gone.** A Session whose status claims a live process while
   its tmux session is absent is lying about itself; that is exactly what an
   audit surface exists to show. Reason: "process is gone but the session still
   records `<status>`".

Unstarted plan rows are *not* attention — a backlog item is not a thing waiting
on you. Attention comes only from Sessions, so it stays a report on what the
machine is actually doing.

`Unavailable` for attention means the Session registry itself failed to read; a
wave with no open sessions returns `ok` with `items: []`.

## Ambient identity

`LF_WAVE_ID` is a wave **id**. Resolve it as one (`store.get_wave`), falling
back to name lookup so an explicit `lf status <name>` and a hand-set
`LF_WAVE_ID=<name>` both work. A UUID that is not in the registry errors with
the id and says the context is stale — never "wave <UUID> is not in the
registry" as though a human typed it.

## Tests (user-facing)

Integration test driving the real `lf` binary against a seeded `LF_HOME`:

- `lf status <wave> --json` carries `runs` and `attention` with the seeded run
  and the blocked session, each with a reason and an age.
- `lf status --json` with `LF_WAVE_ID=<uuid>` resolves the same wave (the
  reproduced failure).
- An empty ledger yields `state: ok, items: []`, not omission.

Plus unit coverage on the attention derivation (dead process, review-owned).

## Scope

Status and its shared read path only. No new store, no Mac/iOS work — the point
is that Swift can decode one `WaveDetailSnapshot` instead of inventing a second
attention model.
