# Select and arrange Work

```sh
lf desktop view show 'wave:product state:open or state:available'
lf desktop dashboard query attention 'state:available'
lf desktop view show 'query:attention'
lf desktop dashboard query attention 'state:available or state:blocked' --revision 1
```

Plans read current Work and saved intent without opening Desktop, allocating a
checkout or creating a Session. JSON includes canonical Work, source occurrences,
retained and visible trees, duplicate winners, titles, states, primary Session,
recorded Machine/cwd and an existing Session connect command. Missing primary or
location remains explicit; no fallback shell is invented. These are host plans,
not a reading of mounted native panes. Saved explicit zoom selects the tile face;
ordinary focus leaves the primary Session as its face.

Repository scope comes from the current repository or `--repo`/`--repository`.
The bounded syntax supports `wave:`, `task:`, `query:`, `kind:task|wave`, and
`state:open|available|busy|blocked|error|unstarted|idle|active|moving|paused|backlog|working|done|auto`
(`available`, `busy`, `blocked`, `error` read Task Sessions; the rest the Task; `auto` is
an open Task whose auto setting is on). `working`,
`backlog` and `done` are the board's sections; `open` is any owned
or published Task not yet finished, including owned Tasks still in Backlog. Whitespace means
AND; OR and parentheses compose expressions. Quote values containing spaces.
Repository is never a predicate. Task IDs, issue identifiers and unambiguous
`task:#123` references resolve to canonical Work. Explicit queries can select
unstarted or completed Tasks. Busy, Blocked and Error come from current reports;
Available uses the shared Session reader's judgment.

`query` atomically saves the expression and reconciled arrangement. Omit revision
only when creating a Dashboard; stale edits fail. Unchanged clauses retain authored
IDs, including when inserting a sibling next to a previously lone clause. A
subsequent plan for `query:ID` uses its saved arrangement. Revisiting an earlier
expression restores its authored occurrences, geometry and hidden members before
reconciling current Work. Whitespace changes do not create another arrangement.
These commands emit JSON;
invalid syntax exits 2, missing/ambiguous selection exits 3, other failures exit 1.
An empty complete result succeeds. Dashboard edits are not retried automatically.
An ambiguous saved query can be repaired by editing its expression. Edits validate
the replacement and its referenced queries; unrelated Dashboards do not block them.

```rust,ignore
let selection = Selection {
    id: SelectionId::new(),
    expression: SelectionExpression::Repository,
};
let resolution = resolve_selection(&store, &repository_id, &selection)?;
let arrangement = Arrangement::automatic(&resolution.members);
let saved = save_dashboard(
    &store, &repository_id, &DashboardId::new(), None, &selection, &arrangement.to_edit(),
)?;
```

Typed selections contain repository/Wave defaults, explicit Work, ordered
composition, intersections, predicates and saved-query references. The repository
default includes started, unfinished Tasks and immediate child Waves; a Wave
default also lists its unstarted Tasks, which offer Start. Task membership
includes descendant Waves. Idle Tasks remain. Explicit references also
reach backlog and finished Tasks. Preserve authored node IDs across edits: occurrences use those source
paths and canonical Work keys, so sibling insertion does not change identity.

`store::selections::resolve_selection` observes Session identity and time outside
SQL, then reads Work and query definitions in one SQLite savepoint after
checking that the saved projection inputs still match.
Task state, publication and ownership come from a fixed set of statements for the
whole repository, not a query per Task. Missing references are explicit
unavailable evidence; database failures return errors. Neither is confirmed
emptiness. `read_dashboard` returns saved intent, current evidence, and a
reconciled arrangement without writing it. Incomplete reads keep earlier
members; complete departures can prune them.

A saved order holds every retained member exactly once, including hidden
members; members fill the slot layout in that order. `save_dashboard` owns
auto/touched: a changed order touches the arrangement, and hide/restore never
does. An automatic order follows the query; a touched one keeps survivors in
place and appends arrivals. Incomplete reads keep the current order. Projection
suppresses later occurrences of the same Work, so hiding a winner promotes the
next. Focus, native surfaces and provider lifetime are outside this contract.

Dashboards atomically own their typed expression and portable arrangement.
Pass the last saved revision when editing or deleting; stale writes fail without
mutation. lfdb stores authored references, ratios, hidden members and auto/touched
intent, plus Dashboard names and pinning. Work titles remain with their owners; there
is no query-result cache, native handle or attachment claim.
The existing Work observer publishes repository defaults, Wave defaults and saved
plans in its planning frame; `RegistryQuery` decodes the same DTO as one-shot
plans. Swift has no query evaluator. Desktop's live pane and window layouts still
belong to their existing native owners.

```sh
lf desktop dashboard pin attention --revision 2
lf desktop dashboard rename query:attention Available --revision 3
lf desktop hide -p query:attention/task:task_33333333333333333333333333333333
lf desktop move -p query:attention/task:task_33333333333333333333333333333333 --destination task:task_11111111111111111111111111111111
lf desktop move -p working-tasks/task:task_33333333333333333333333333333333 --destination task:task_11111111111111111111111111111111
lf desktop dashboard slots query:attention --revision 6 < slots.json   # null restores the default slots
lf desktop layout query:attention                                      # read the Dashboard's slots
```

A Dashboard has its own `DashboardId`, name, query and arrangement. Two Dashboards
can select the same Tasks independently while reusing each Task's pane and draft
record. `query:ID` references that Dashboard's selection, including in nested
expressions. Repository and Wave defaults remain built in; Dashboard edits never
replace them. Desktop saves each Work's pane layout itself, under
`$LF_HOME/desktop-cache/pane-layouts/<Machine>/<kind>/<Work ID>.json`; a
Session pane naming a Session the Machine no longer lists opens empty.
Pin/unpin and Dashboard rename require the current Dashboard revision. Task rename uses
`task:ID` and the Task's revision, writing through the Task update owner.
Wave renaming remains `lf wave rename`. All saved-intent commands work headless.

`hide`, `show`, `move` and `resize` aimed at a tile edit the shared record: a
Dashboard's arrangement and slots, or the repository's one Task order behind
Working Tasks and Wave views. `-p query:ID/TILE` and `-p working-tasks/TILE` name
the tile by Work key and need no Desktop; `-p 3` names the third tile of the view
on screen. Each edit reads the current revision; `move` gives the tile the
destination's place, hiding keeps its position and never changes `mode`, and
`resize` reweights the innermost slot split between two tiles.
Desktop consumes the same plans in native Dashboards and `desktop view show`.
`desktop view EXPRESSION` routes through the repository window; `desktop state`
includes its selected view and actual geometry. Saved shell intent never
authorizes a launch.

## Promote existing work

```sh
lf task create --session SESSION --title "Finish prototype"
lf session bind SESSION --task LOO-12
```

Select `--session` to keep the chat and linked checkout, or `--worktree` for
the checkout alone. Plain `lf task create` files new work; `--run` starts it
without implicitly adopting anything. `lf session list --json` supplies
conversation IDs. See
[Work](../../../../docs/work.md#adopt-work-already-in-progress) for shared checkouts,
existing-Task continuation and recovery.
