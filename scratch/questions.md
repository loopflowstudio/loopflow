# LOO-1114: choices made with nobody here

- Titles read only for terminals with a viewer, through the existing per-Session summary read with cached statements, not a new batched title query. Pass 1 measures the viewed-set sweep; the batched query is the fallback.
- Viewers keep reading SQLite behind a revision gate; server-pushed `Event::Title` (fixes remote cmux/tmux renames, needs protocol 4 → 5) is deferred in the plan. The post-build review recommends doing it in this PR (its Q1a). Protocol bumps can't be taken back once released, so it waits for a person: this Task, or its own?
- No liveness clock for "Available" after an agent dies silently: `recovery-wake` writes within 60 s and that wakes the reader.

## Ship review (2026-11-26, nobody present)

- Filed LOO-1115 (server-pushed titles, protocol 4 → 5, plus the launch-time `Left “…”` line) and 1e7e41 (recovery-wake and lifetime watcher onto the shared observer). No reply recorded; questions 1–4 in `scratch/ship-review.html` stay open.
