# Project 1: Protocol Alignment

Fix the event namespace and field name mismatches between Concerto and lfd.

**Status:** Complete

---

## Problem

Concerto subscribes to events but never receives them due to naming mismatches.

### Event Namespace Mismatch

| Side | Pattern | Events |
|------|---------|--------|
| Swift subscribes | `session.*` | expects `session.started`, `session.ended` |
| Python emits | — | sends `step_run.started`, `step_run.ended` |

**Result:** Session events never reach Concerto.

### Field Name Mismatch

In `output.line` events:

| Side | Field |
|------|-------|
| Swift expects | `session_id` |
| Python sends | `step_run_id` |

**Result:** Output lines can't be associated with sessions.

---

## Solution

Align Python to emit what Swift expects. This is simpler than changing Swift because:

1. Swift naming (`session`) is more user-facing
2. Python is the source of truth—it should speak the client's language
3. One place to change vs. multiple Swift files

### Changes Required

**`server.py`** — Change event names:
```python
# Before
Event("step_run.started", {...})
Event("step_run.ended", {...})

# After
Event("session.started", {...})
Event("session.ended", {...})
```

**`server.py`** — Change field name in output events:
```python
# Before
{"step_run_id": step_run_id, "text": text, "timestamp": ...}

# After
{"session_id": step_run_id, "text": text, "timestamp": ...}
```

### Files to Modify

1. `src/loopflow/lfd/daemon/server.py` — Event emission
2. `src/loopflow/lfd/daemon/protocol.py` — Document the protocol (if not already)

### Files to Verify (Swift)

No changes needed, but verify these parse correctly after fix:

1. `swift/LoopflowCore/Services/LFDEventService.swift` — Event parsing
2. `swift/Concerto/AppState.swift` — Event subscription patterns

---

## Testing

1. Start lfd: `lfd serve`
2. Open Concerto, verify "lfd connected" status
3. Run a step: `lf review`
4. Verify Concerto shows:
   - Session appears in UI
   - Output streams in real-time
   - Session completes with correct status

---

## Done When

- [x] `session.started` events received by Concerto
- [x] `session.ended` events received by Concerto
- [x] `output.line` events associated with correct session
- [ ] No protocol-related errors in Concerto logs (verify manually)
