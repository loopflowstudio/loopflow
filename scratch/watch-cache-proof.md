# Recent Watch views — 2026-09-24

WorkspaceNavigation retains four recently viewed Tasks per repository/window.
Revisiting promotes the existing store. A fifth distinct Task releases the least
recent store, including its snapshot, transcript and cursors. Reopening uses the
existing mounted Watch reads and starts at current progress with filters cleared;
saved history remains available through Load history. README states that limit.

## Focused proof

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'WorkspaceNavigationTests/(boundedWatchRetention|watchContentAndRetention)'
```

Two tests pass; Mac compilation/linking passes. Final receipt:
`/tmp/loo293-watch-cache-tests.log`. The new test loads four stores from shared
fixtures, revisits one, observes the evicted store's weak reference becoming nil,
checks retained filters/output, and reopens the discarded Task. Its reader gives
early history only on a fresh read, proving recovery without an old continuation.
Follow preserves the two displayed rows without duplication. The existing view
proof covers repository isolation, terminal layout and removal of hidden Watch.

The initial test had incorrect fixture expectations: it selected a stage under
the active invocation instead of the output's earlier invocation, and omitted
the command text from the tool row. Corrected those expectations; no production
change was needed. No executable edits followed the passing command.

## Review

One ordered collection replaces the dictionary; there is no second cache,
selection tombstone, durable transcript, new query, or Session action. Conditional
Watch mounting still owns read cancellation. Terminal surfaces and pane ownership
remain independent. Review made the lost filters/current-stage behavior explicit
in documentation rather than implying unlimited navigation retention.

This is fixture/model/view proof, not measured process memory or configured UI
interaction. The cache bounds store count per repository/window. Other windows,
repositories, sheet views and finishing cancelled reads have separate lifetimes.
Source/snapshot inventories, incoming decode allocation, Rust discovery, tail
initialization and cursor growth still need bounds before automatic polling.
Complete capture, exact checkpoint Session navigation and the configured human
demo remain in this Task/PR. No broad gate, publication or settlement was run.
