# 05: Tools + Real Turn Loop

Turn the skeleton into a working tool-calling agent loop.

## What exists after this

- memory tools (`replace/insert/rethink/delete`)
- `send_message` tool with `progress`/`final`
- wave tools (`read_file`, `write_file`, `shell`) in ephemeral workspace lane
- completion contract enforced in runtime

## Commit slices

### C1 — Memory + send_message tools (~300-500 LOC)

- parse tool args
- apply/persist memory edits per call
- emit explicit message events by phase

### C2 — File + shell tools (~300-550 LOC)

- wave-scoped read/write/shell handlers
- isolate writes to ephemeral lane workspace
- return structured tool results to model

### C3 — Integrate tools into guarded loop (~300-500 LOC)

- dispatch tool calls
- push tool results back into model context
- end only when exactly one final message exists and no follow-up needed

## Constraints

- Progress messages are explicit tool calls, not status side channels.
- Final message required on successful turn.
- Filesystem changes must remain ephemeral by default.

## Done when

```bash
cargo test -p lf-agent tools turn_loop
```

Expected: tool and completion tests pass.
