# Data model — synthesis for Jack

2026-09-26. Presented at https://claude.ai/artifact/5QiVvD9jBQH7uVrZw2fE8b
(private). Sources: [product-first review](data-model-concepts.md),
[infrastructure inventory](data-model-review.md), Jack's decisions in
[demo-native-workspace.md](demo-native-workspace.md).

## The model as Jack uses it

Repository → Wave → Task → (one active PR; Flow invocations, one current).
Run has three nullable parents, `invocation ⇒ task ⇒ wave`, plus node and
iteration tuple when in an invocation; the constructor fills upward and
refuses a mismatch. A Flow invocation owns the Runs its driver launched and
has a nullable `parent` invocation for runtime nesting only (a loop body
entered on a pass); template composition stays definition-time and every
invocation is fully unrolled; there is no "occurrence" object and no
path-string node key. Session is a child row of one Run; its
Task, Wave, provider and Flow position are read through the Run; bind
changes the Run's task, never its invocation.

## What the code does instead (verified today)

- No Session record: four projections over four stores, three id schemes,
  one `session list`.
- Run→Task is a `task:` string in a ranked subject list with a two-value
  source, written once at launch, mirrored into `task_events.started`.
- Reads go through the launch resolver, which requires an active PR — a
  bound Session becomes an orphan when its PR merges (`ops/run.rs:172-176`).
- Name is a sidecar (`session-name.json`) copied on Run replacement.
- `runs --task` scans every manifest and rebuilds a selector catalogue.
- Swift regroups Sessions under Tasks on every `model.workspace` access.
- SQLite `runs` table has the Task FK and no writer outside migrations.

## Simplifications, ranked

1. Typed nullable `task`/`wave` + `work_source` on Run; delete the string
   encoding, ranking, Project leak, double write; ship the validators.
2. Session = Run with a conversation; name + provenance on the record; delete
   the sidecar and `carry_session_name`.
3. Reads never call the launch resolver.
4. One Task→Runs index (write the `runs` table or a per-Home index); derive
   `started`; enable `session list --task`.
5. Swift reads fields; cache `WorkspaceProjection`; delete `subject(for:)`.
6. Universal `lf session bind` as a field write with a Rust-owned `Bind`
   action.
7. Wire cleanup: drop `wave_id`, `work_path`; decide old-capture Optionals.
8. S2 orphan predicate becomes `task == null`.

## Decided by Jack, 2026-09-26

Main objects are main tables. `runs` is written as the Run record; `sessions`
is its child (`run_id` FK, kind, title + provenance, state); Flow review and
Ask Sessions become rows keyed by `session_run_id`; the manifest stays
launch evidence; every sidecar goes. This closes row-vs-manifest, one id
scheme, and the `runs` table.

## Still open for Jack (defaults in the page)

Where `started` comes from (default: derived from Runs); old Runs rewrite or
null (default: one-time rewrite); bind to a landed/done Task (default:
bind); usage attribution after bind (default: follows the field); names on
headless Runs (default: allowed).
