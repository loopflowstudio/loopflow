# Open questions / blockers

## Asana token expired (operational, blocks PM refresh)

`lf op ingest` fast path failed:

```
warning: failed to pull PM items for wave/Desktop: Stored asana token has expired. Run `lf op auth asana` again.
Error: no roadmap items in wave/Desktop/
```

The "no roadmap items" message is misleading — `wave/Desktop/` does contain
`1-embedded-terminal-build-driver.md` and `2-native-chat-ux.md`. The real
failure is the expired Asana token blocking the PM mirror refresh.

**Action needed (human):** run `lf op auth asana` to restore PM sync. Until
then, ingest/PM-backed flows operate on the stale local mirror only.

## Ingest decision (assumption, headless)

Per the ingest step's documented fallback ("if the pull fails, ingest warns and
falls back to the local `wave/<name>/` mirror"), I fell back to the local
mirror instead of failing.

State at ingest time:

- `scratch/Desktop-native-chat-ux.md` already exists, committed in
  `12945db5 lf commit: ingest`, with a valid claim
  (`claimed_by: f6684ab5-...`, `claimed_at: 2026-05-19T02:01:39Z`,
  `asana_id: 1214270115439574` matching `wave/Desktop/2-native-chat-ux.md`)
  and `status: in-progress`.
- Priority 1 (`embedded-terminal-build-driver`) outranks the selected
  priority-2 item, but every local signal says it is the actively-worked p1
  task under a separate claim: wave MEMORY records embedded-terminal
  implementation verified 2026-05-19, and recent commits
  (`6b300ab1 desktop: clean palette terminal exit files`) are
  embedded-terminal work. This is consistent with why the prior ingest
  selected native-chat-ux over it.

**Assumption:** ingest is already satisfied. I left
`scratch/Desktop-native-chat-ux.md` intact rather than clobbering an
in-progress claim or re-picking a higher-priority item that appears claimed by
active work. With Asana unreachable the claim on item 1 cannot be re-verified;
if it is in fact free, a human (or a post-`lf op auth asana` ingest) should
re-pick.
