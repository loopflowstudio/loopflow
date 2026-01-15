# History

Migrate session logging from `maestro.db` to `lfd` daemon, enabling worktree-based history queries and live UI updates in Maestro.

## Review

**Verdict:** Needs work

### Duplicate Session models

Two `Session` classes now exist:
- `loopflow.maestro.session.Session` with `backend` field and `Path` types
- `loopflow.lfd.models.Session` with `model` field and `str` types

The old maestro module still exports `Session` and `SessionStatus` via `__init__.py` and remains used by `lfops.py`, `cli/sessions.py`, and other files. The migration is incomplete—either remove the old model and update all consumers, or keep one source of truth.

### Incomplete migration

Several files still import from the old location:
- `src/loopflow/lfops.py` uses `maestro.db` functions directly
- `src/loopflow/cli/sessions.py` imports `from loopflow.maestro`
- `src/loopflow/maestro/collector.py`, `runner.py` still use old paths

The diff migrates `cli/run.py` and `pipeline.py` but leaves these others. Either complete the migration or revert to a consistent state.

### Memory safety in Swift sqlite binding

In `SessionService.swift:64`:
```swift
sqlite3_bind_text(stmt, 1, value, -1, nil)
```

Passing `nil` as the destructor tells SQLite the string is static and won't be freed. Swift strings are not static—they can be deallocated while SQLite still references them. Use `SQLITE_TRANSIENT` to make SQLite copy the string:
```swift
sqlite3_bind_text(stmt, 1, value, -1, unsafeBitCast(-1, to: sqlite3_destructor_type.self))
```

Or use the common pattern of binding within a `value.withCString` block.

### Worktree removed Codable conformance

`Worktree.swift:22` changed from:
```swift
struct Worktree: Identifiable, Codable, Hashable
```
to:
```swift
struct Worktree: Identifiable, Hashable
```

This breaks any code that serializes `Worktree` (caching, etc). If `Codable` was removed intentionally because `TaskSession` isn't synthesizable, add manual `Codable` conformance or reconsider the design.

### Missing tests

No tests for:
- `sessions.start`, `sessions.end`, `sessions.history` server methods
- Fire-and-forget client functions
- Swift `SessionService` queries

The existing `test_maestro_db.py` tests the old path. Add tests for the new lfd session functions.

## Design notes

The architecture (lf -> lfd -> Maestro) is sound. Key constraints:
- Fire-and-forget logging so task execution never blocks on daemon
- lfd must be running for history; graceful fallback if not
- Maestro reads directly from `lfd.db` rather than querying the socket (simpler for read-only queries)
