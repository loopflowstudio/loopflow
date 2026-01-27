# Open Questions

## reduce-big (2026-01-25)

1. **Interactive session direction**: The embedded Ghostty terminal work is in flight. Should simplification wait until that lands, or should the recommendation inform whether to continue that direction?

2. **Flow primitives usage**: Are there internal users (the team) actively using `Fork` and `Choose` in flows, or are these purely speculative features? Usage data would validate whether these are truly unused.

3. **Watch/cron demand**: Has anyone requested schedule-based or file-watch triggers? If so, that changes the recommendation on stimulus types.

## jack-heart.ghost.20260125_1034 (2026-01-26)

1. **Closing Concerto during session**: Current behavior kills the terminal process. Is this the desired behavior, or should sessions survive app restart?
