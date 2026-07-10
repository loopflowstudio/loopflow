## Try it!

Confirm that a one-shot request never creates loop state or looks up a wave:

```bash
cargo run -p loopflow --bin lf -- loop task "work" --wave not-registered --max-passes 1
# Error: --max-passes must be at least 2; use `lf flow task "<seed>"` for one-shot work
```

Exercise flag ownership, tiered prompt policy, trace handoff, and the full gate:

```bash
cargo test -p loopflow --bin lf reorder_args
cargo test -p loopflow execution_context_grants_delegation_by_tier
cargo test -p loopflow sibling_loops_under_one_parent_trace_get_distinct_run_ids
uv run python scripts/test.py --all
```

The universal operating document shrinks from 9,147 to 4,707 bytes (48.5%)
while tier skills retain the orchestration detail they need.

## Intent

Make Loopflow execute assigned work in the current process by default and pay
for a child loop only when a strict subset needs its own repeated lifecycle or
useful parallelism. At the same time, make placed work observable as one thing:
the registry run id, trace id, prompt evidence, and work-line reports all agree.

## Assumptions

- A top-level `--wave` names an existing row in the local wave registry; unknown
  explicit identities should fail rather than fall back to ambient context.
- `run_id` is the trace and `process_id` is one process span. A placed run is a
  new trace root; each pass creates a fresh span inside it.
- Detached loops require an already-served wave and at least two available
  passes. One-shot work belongs on `lf flow`.
- Local command flags win spelling collisions with global flags, and `--` ends
  normalization.

## Key decisions

- Keep universal prompt guidance small; grant PM and loop capabilities in the
  wave/project/task skills that exercise them.
- Derive CLI flag tables from Clap and move flags only to an owner on the
  selected command path.
- Validate the loop lifecycle before wave or placement resolution.
- Mint the placed id once, pass it through detached HTTP launch, clear inherited
  process identity, and route reports on `wave.<short-run-id>`.

## Not included

- No automatic server startup, PM/auth repair, or approximate wave inference.
- No blanket rejection yet for every command that parses but ignores placement
  flags.
- No eval runner, PR delivery record, or escalation event work.
