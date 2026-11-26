# LOO-1114: choices made with nobody here

- Titles read only for terminals with a viewer, through the existing per-Session summary read with cached statements, not a new batched title query. Pass 1 measures the viewed-set sweep; the batched query is the fallback.
- Viewers keep reading SQLite behind a revision gate; server-pushed `Event::Title` (fixes remote cmux/tmux renames, needs protocol 4 → 5) is deferred in the plan. Worth doing in this Task instead?
- No liveness clock for "Available" after an agent dies silently: `recovery-wake` writes within 60 s and that wakes the reader.
