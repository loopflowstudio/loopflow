# Wave ancestry & chord structure — design

Roadmap item: `wave/goals/03-wave-ancestry-chord-structure.md` (was `2-wave-ancestry.md`).

**Finish line:** `Wave` carries its parent/child relation again, `WaveAgentTree.child_waves`
is populated, a chord's contents are just its children, and a two-repo chord runs with
one child Looping Agent per repo.

---

## 0. Correction to the premise (verified against current source)

The item and `MEMORY.md` both assert: *"The store schema still has `parent_wave_id`
columns."* **That is no longer true.** The migration history undid it in two steps:

- `011_chords_data_model.sql` — recreated `waves` with `wave_type`, `parent_wave_id`,
  `position` (self-referential model) + index `idx_waves_parent`.
- `013_remove_chord_tree.sql:5-7` — `ALTER TABLE waves DROP COLUMN wave_type / parent_wave_id
  / position`, and moved membership into standalone `chords` + `chord_members` join tables.
- `028_drop_chords_tables.sql` — dropped `chord_members`; the join tables are gone too.

So today's `waves` table has **no ancestry column of any kind**, and there are no chord
tables. Current columns (from the query templates in `store/catalog.rs:177-197`):
`id, name, direction, area, paused, created_at, workers, mode, primary_flow, goal, metrics`
(plus dormant `schema_ref`, `schema_name`, `serialized`). Waves are stored **column-based**,
not as a JSON blob — every SELECT lists explicit columns, so a new field means touching the
schema and every wave query, not just a serde struct.

Consequence for this work: we **add a migration** that reintroduces `parent_wave_id`. We are
not "surfacing an existing column." Everything else in the item stands.

`member` vocabulary is clean: the only surviving hit in wave-structure code is
`store/migrations.rs:413`, an assertion that the `chords`/`chord_members` tables are *absent*.
Nothing to purge.

---

## 1. The regression, precisely

Two drops, one visible symptom.

- **The durable type lost its parent field.** `struct Wave` (`lfd/types/wave.rs:229-254`)
  has `id, name, mode, primary_flow, goal, metrics, crons, repos, direction, area, paused,
  created_at, workers` — no parent. `map_wave_row` (`store/rows.rs:103-130`) reads eleven
  columns and constructs `Wave` with no ancestry.
- **The tree route hardcodes empty children.** `get_wave_agent_tree_handler`
  (`lfd/http/routes/waves.rs:833-839`) returns:

  ```rust
  Ok(Json(WaveAgentTreeDto {
      object: "wave_agent_tree".to_string(),
      id: format!("tree-{wave_id}"),
      wave: root_wave,
      child_waves: Vec::new(),   // <- line 837, never populated
      sessions,
  }))
  ```

  The test at `waves.rs:2419` pins this: `assert_eq!(response.child_waves.len(), 0)`.

`WaveAgentTreeDto.child_waves: Vec<WaveDto>` (`http/dto.rs:335`) is a real, wired field — its
Python (`models.py:166`) and Swift (`Session.swift`) mirrors exist. It is simply always fed
an empty vec because the durable model can no longer answer "who are this wave's children?"

---

## 2. Design

Minimal, self-referential, query-at-runtime. One scalar field, one migration, one store
query, one route change.

### 2a. `parent_wave_id` on the durable type

Add to `struct Wave`:

```rust
/// Parent wave in the chord tree. `None` for a root wave. A chord is simply a
/// wave that has children (query at runtime); there is no `wave_type` column.
pub parent_wave_id: Option<LfdId>,
```

`Wave::new` sets `parent_wave_id: None`. A `with_parent(mut self, parent: LfdId) -> Self`
builder (or a plain setter used at chord-creation time) attaches a child.

**No `wave_type` column.** The item phrases a chord as `wave_type = chord`, but reintroducing
a discriminator column duplicates information the tree already carries: *a chord is a wave
whose `children_of(id)` is non-empty.* Deriving chord-ness from the presence of children
keeps a single source of truth and honours the item's "no separate chord entity, no
denormalized cache." If a UI later needs to mark an *empty* chord (a parent with no children
yet), revisit — but not on this pass.

**No `position` column.** Sibling order was part of the old model; nothing needs it now.
Order children by `created_at` (stable, already indexed) until a real ordering requirement
appears.

### 2b. Migration `044_wave_parent.sql`

```sql
ALTER TABLE waves ADD COLUMN parent_wave_id TEXT
    REFERENCES waves(id) ON DELETE CASCADE;
CREATE INDEX idx_waves_parent ON waves(parent_wave_id);
```

`ON DELETE CASCADE` mirrors the original 011 semantics: deleting a chord deletes its children.
The column is nullable — a root wave has no parent — which is the correct wire shape (see §4;
`Option`, not a defaulted sentinel).

### 2c. Store: read/write the column + a `children_of` query

- `map_wave_row` (`rows.rs:103`): read the new column as `Option<LfdId>` (add to every wave
  SELECT's column list). The five templates to extend in `catalog.rs`: `list_waves`
  (177/182), `get_wave` (192), `get_wave_by_name` (197), and the `INSERT ... ON CONFLICT`
  upsert (187). `list_loopable_waves` shares the list-wave shape — extend it too.
- Insert/upsert param binding in `store/sqlite.rs` + `store/postgres.rs` gains one bound
  value (`wave.parent_wave_id`).
- New query + `WaveStateStore` method:

  ```rust
  async fn children_of(&self, parent: &LfdId) -> StoreResult<Vec<Wave>>;
  // SELECT <wave cols> FROM waves WHERE parent_wave_id = {p1} ORDER BY created_at ASC
  ```

  Expose it through the store facade (`store/mod.rs`) as `list_child_waves(parent)` and
  reuse the existing `attach_repos_vec` stitch so each child arrives with its `repos`
  populated (same pattern as `list_waves`).

### 2d. Populate `WaveAgentTree.child_waves`

In `get_wave_agent_tree_handler` (`waves.rs:796`), between loading `root_wave` and building
the response:

```rust
let children = state.store.list_child_waves(&wave_id).await.map_err(map_store_error)?;
let child_waves = build_wave_dtos(&state.store, &state.github, children, false).await?;
```

Feed `child_waves` into the `WaveAgentTreeDto` instead of `Vec::new()`. `build_wave_dtos`
(`routes/mod.rs:60`) already exists and maps a `Vec<Wave>` to `Vec<WaveDto>`. Flip the pinning
test at `waves.rs:2419` to assert the chord's real child count.

Vocabulary stays **parent / child / sibling** throughout. No "member."

---

## 3. Cross-repo Goals fall out for free

A cross-repo Goal is a **chord whose children live in different repos.** Nothing extra:

- Each leaf child wave keeps its single `repos` entry (`wave.repo()` stays single per leaf).
- The parent chord spans whatever its children span — it holds no repo of its own; its reach
  is the union of its children's.
- Running it: the executor already launches one wave-agent Session per wave via
  `launch_wave_agent_session` (`executor/wave/mod.rs:390`), each in that wave's own worktree
  with `session_use: WaveAgent`. A two-repo chord = a parent with two leaf children; looping
  the chord launches one wave-agent (Looping Agent) per child, i.e. **one per repo.** That is
  exactly the item's Done-when line, and it needs no new launch machinery — only the ancestry
  query to know which children to loop.

**Open fork — do not resolve here.** Item `3-wave-repo-split` proposes a single *leaf* wave
spanning repos directly via `repos: [RepoWork]` (many worktrees, coordinated cross-repo PRs).
That is the alternative answer to cross-repo Goals — atomicity inside one agent vs. one agent
per repo under a chord. Land ancestry first; it unblocks the chord model regardless of how the
fork resolves. Note that `Wave.repos: Vec<RepoWork>` already exists (the plumbing for `3` is
partly present), so the two designs are not mutually exclusive at the type level — but the
default remains **chord-spanning only** until atomicity proves necessary.

---

## 4. Migration / DTO impact (crosses the wire)

`parent_wave_id` surfaces on `WaveDto`, which is mirrored in Rust / Python / Swift. Per the
CLAUDE.md DTO rules (no defaults on wire types):

- **Rust** `WaveDto` (`http/dto.rs:77`): add `pub parent_wave_id: Option<String>`. Genuinely
  optional (root waves have none) → `Option`, serialized as explicit `null` or via the
  existing `skip_serializing_if = "Option::is_none"` convention already used on that struct.
  No `#[serde(default)]`, no `Default`.
- **Python** `Wave` (`loopflow/models.py:97`): `parent_wave_id: str | None` — no Pydantic
  field default beyond the Optional itself.
- **Swift** `Wave` (`LoopflowCore/Models/Wave.swift:239`): `let parentWaveId: String?`, parsed
  as `T?` with no `?? value` fallback.
- **`WaveAgentTree.child_waves`** already exists in all three mirrors; only its Rust producer
  changes. No new tree DTO field.

**Fixture round-trip obligation** (`tests/dto_fixtures.rs`): the harness parses
`tests/fixtures/wave.json` in Rust (`wave_fixture_nests_repo_work`), Python, and Swift. Adding
a DTO field means:
1. Add `parent_wave_id` to `tests/fixtures/wave.json` (a value for a child; the root/other
   fixtures can carry `null`).
2. Assert it in `dto_fixtures.rs`, the Python fixture test, and the Swift fixture test.

If a `wave_agent_tree.json` fixture is wanted to pin a non-empty `child_waves`, add it under
`tests/fixtures/dto/` and to all three suites — optional but the natural place to lock the
regression fixed.

---

## 5. Build plan + test plan

Build, bottom-up (each step compiles before the next):

1. **Migration** `044_wave_parent.sql` — column + `idx_waves_parent`. Confirm
   `migrations.rs` picks it up (the ordered-list mechanism).
2. **Type** — `parent_wave_id: Option<LfdId>` on `Wave`; `Wave::new` → `None`; child-attach
   helper.
3. **Store read** — extend the five wave query templates + `map_wave_row`; extend insert/
   upsert param binding in both `sqlite.rs` and `postgres.rs`.
4. **Store query** — `children_of` template + `WaveStateStore::children_of` + facade
   `list_child_waves` (with `attach_repos_vec` stitch).
5. **Route** — populate `child_waves` in `get_wave_agent_tree_handler` via `build_wave_dtos`.
6. **DTO mirrors** — `WaveDto.parent_wave_id` in Rust/Python/Swift; update `wave.json` fixture
   + the three fixture tests.

Test, mapped to the item's Done-when:

- **`Wave` exposes its parent relation.** Unit test: create parent + child through the store,
  reload the child, assert `parent_wave_id == parent.id`.
- **Chord contents = children where `parent_wave_id = id`.** Store test on `children_of`:
  parent with two children in different repos returns both, ordered; a leaf returns empty.
- **`WaveAgentTree` returns child waves (not empty).** Flip `waves.rs:2419` from
  `child_waves.len() == 0` to the real count; add a case with a chord + two children asserting
  each child's repo appears.
- **No "member" in wave-structure code.** Keep the `migrations.rs:413` guard; grep stays clean.
- **A two-repo chord runs with a child Looping Agent per repo.** Executor/integration test:
  build a chord with two leaf children (repos A and B), loop it, assert one wave-agent Session
  per child (`session_use: WaveAgent`, one worktree per repo) via `launch_wave_agent_session`.
- **`cargo test` passes** — plus the Python and Swift fixture suites (three-mirror round-trip).

### Open questions for the author
- Confirm dropping `wave_type`/`position` for good (derive chord-ness from children, order by
  `created_at`) vs. reintroducing a discriminator. Design above assumes derive.
- Whether a root-wave `parent_wave_id` serializes as explicit `null` or is omitted via
  `skip_serializing_if` — pick one and keep all three mirrors in lockstep (fixture pins it).
