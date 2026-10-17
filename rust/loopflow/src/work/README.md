# Select and arrange Work

```rust,ignore
let selection = Selection {
    id: SelectionId::new(),
    expression: SelectionExpression::Repository,
};
let resolution = store.resolve_selection(&repository_id, &selection)?;
let arrangement = Arrangement::automatic(&resolution.members);
let saved = store.save_selection(
    &repository_id, &QueryId::new(), None, &selection, &arrangement,
)?;
```

Typed selections contain repository/Wave defaults, explicit Work, ordered
composition, and saved-query references. Defaults include started, unfinished
Tasks and immediate child Waves; Task membership includes descendant Waves.
Paused and idle Tasks remain. Explicit references also reach backlog and finished
Tasks. Preserve authored node IDs across edits: occurrences use those source
paths and canonical Work keys, so sibling insertion does not change identity.

`resolve_selection` reads Work and query definitions in one SQLite savepoint.
Missing references are explicit unavailable evidence; database failures return
errors. Neither is confirmed emptiness. `read_selection` returns saved intent,
current evidence, and a reconciled arrangement without writing it. Incomplete
reads keep earlier members; complete departures can prune them.

`Arrangement::replace` requires every retained member exactly once, including
hidden members. Hide/restore changes visibility, preserving edits to the current
tree. Projection suppresses later occurrences of the same Work in traversal order;
hiding a winner promotes the next. Auto arrangements follow selection order;
structural edits mark them touched, preserving survivor geometry and appending
arrivals. Focus, native surfaces and provider lifetime are outside this contract.

Saved selections atomically own their typed expression and portable arrangement.
Pass the last saved revision when editing or deleting; stale writes fail without
mutation. lfdb stores authored references, ratios, hidden members and auto/touched
intent. It stores no titles, query-result cache, native handles or attachment claims.
Desktop's live pane and window layouts still belong to their existing Swift owners;
query UI and command grammar will consume this shared contract separately.
