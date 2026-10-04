# Wave journal recovery

`queued-after-skip.jsonl` preserves a failed attempt, a historical skip, nested
and root queues, captured Skill content, provider-session attribution and later
unconsumed feedback. Replay it without a source catalog and attempt startup
twice: one unresolved cutover receipt prevents execution, failures stay failed,
queued work keeps its order and return relationship, and the original journal
bytes remain intact. Explicit cancellation preserves that evidence too.

This is a synthetic journal using the pre-cutover format. Keep it fixed when
removing the Wave interpreter; migrate its reader and recovery assertions instead
of regenerating the fixture from the replacement runtime.
