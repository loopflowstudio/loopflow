# Subwaves: Wave name as identity (Infrastructure · LOO-329)

Status: **in review-design with Jack Heart, 2026-09-28.** The model below
follows Jack's direction in that session. Items marked *open* are not decided.
No implementation, registry write, Wave split, or ownership migration follows
from this document.

Review notes: [subwave-identity-review.md](subwave-identity-review.md).
Open questions: [questions.md](subwave-questions.md).
The kickoff draft this replaces is at commit `0381a195f`.

Companion designs: Product
[LOO-330](https://linear.app/loopflow/issue/LOO-330) (address grammar,
navigation, `lf memory` reads) and Intelligence
[LOO-331](https://linear.app/loopflow/issue/LOO-331) (what a prompt includes).
Both were drafted against the kickoff model and use its words "held scope" and
"address history". Those concepts are removed here; both drafts need the
correction.

## Base: LOO-298

Jack Heart directed on 2026-09-28 that this work rebase onto
[LOO-298](https://linear.app/loopflow/issue/LOO-298), PR
[#1296](https://github.com/loopflowstudio/loopflow/pull/1296), branch
`jack-heart/data-model-one-table-per` at `d07e56933`. That branch is three
commits behind main and conflicts with it. The earlier tip is kept at
`lf-backup/subwave-pre-298`. Source evidence in "What the code does today" was
read at `632b67ec7`; the rows below were re-read on the new base.

| On LOO-298 | Effect on this design |
|---|---|
| Runs, agent Sessions and Flow Sessions are SQLite rows with a `wave_id` reference | History is no longer only files. A rename must say what happens to those rows. See Rename. |
| Triggers validate Run → Task → Project → Wave agreement | The validator pattern `tasks.wave` needs already exists. |
| `tasks` still has no Wave column | `tasks.wave` remains an addition. |
| `lfd`, residents, Wave listeners, webhook receivers and desktop Wave chat are removed | Chat journals, endpoint files, resident worktrees and the promotion wake leave the rename problem. Surfaces with no Task shrink to cron, finite `wave/operate` Runs and the app's Wave page. |
| No Chapter table; a Chapter is the shared name of each Wave's In Progress Linear Project; `lf repo new-chapter` | Release as a Wave gets one Project carrying the shared chapter name. |
| `docs/architecture-reference.md` states the Wave UUID is durable identity, stable across rename and rehome; `lf wave relocate <uuid>`; `pm_snapshots` keyed by UUID | This design reverses a written contract. LOO-298's method is docs first, so slice 4 starts by rewriting those lines. |
| `wave_id` appears 551 times in Rust | Same scale of migration. |
| The memory chain still walks `parent_wave_id` in `work/wave/context.rs` | Slice 1 is unchanged. |
| Cron jobs run in a recorded `working_directory`, the repository given at sync | Unchanged. |
| LOO-298's `scratch/` holds about 705 KB of Markdown | Every Run on this branch now carries it, because `scratch/` is read recursively. |

## Problem

Jack Heart selected `wave/infrastructure/release/MEMORY.md` for release
findings and asked for a clear addressing system for the memory store. Today
that file reaches no prompt, and nesting `release/` under `infrastructure/`
confers nothing:

- Discovery requires `GOAL.md`. Release has none, so it is not a Wave.
- Memory inheritance follows the registry's `parent_wave_id`. Nothing in
  production writes that column.
- A Wave has two identifiers: a random UUID minted on one Home, and a name.
  Every surface a person uses speaks the name. The UUID exists so that a rename
  can preserve identity, and that is a feature Jack does not want.

## The model

**A Wave's name is its identity. The name is its path under `wave/`. A
subwave is a Wave whose name begins with another Wave's name.**

| Decision | Source |
|---|---|
| Wave id = Wave name. No separate Wave id. | Jack Heart, review-design 2026-09-28 |
| Renames do not preserve identity. A new name is a new Wave. | Jack Heart, same session |
| Subwaves are treated as separate Waves in most ways. | Jack Heart, same session |
| A Task records its Wave's name directly. Deriving it through the Project saves rename work, and renames are rare, so that is the wrong tradeoff. | Jack Heart, same session, reversing his earlier "maybe" |
| A Wave rename updates the name wherever current state records it. | Jack Heart, same session |
| Parent is implicit in the name. `parent_wave_id` is dropped. | Jack Heart, same session: "the parentage is implicit in the name" |
| Context flows down the path: every top-level `.md` of the Wave's directory and each ancestor's, memory included. No children, no siblings. `scratch/` stays recursive. | Jack Heart, same session |

### What a subwave shares with its parent

A subwave has its own objective, chapter Project, Tasks, Linear Initiative,
crons, chat and memory file. Two things cross the boundary:

1. **Context flows down only.** A Run at `infrastructure/release` receives
   Infrastructure's Wave file and memory, then release's. A Run at
   `infrastructure` receives nothing from `infrastructure/release`. No Run
   receives a sibling's files. Jack Heart stated this on 2026-09-28 with "i
   think"; LOO-331 owns the detailed rule.

   | Run at | Wave file | Memory | Approx. tokens |
   |---|---|---|---|
   | `infrastructure` | own | own | 18,900 |
   | `infrastructure/release` | parent's, then own | parent's, then own | 20,000 plus release's Wave file |

   Jack restated the rule as "just wave/mywave/* and everything of your
   parents (not your siblings)". So the unit is the directory: every top-level
   `*.md` in the Wave's own directory and in each ancestor's, root first.
   "Top-level" carries the sibling rule, because a sibling is a subdirectory
   of the parent: at `infrastructure/release`, `wave/infrastructure/*.md` is
   read and `wave/infrastructure/auth/` is not. This is today's gatherer
   applied once per ancestor, with `MEMORY.md` no longer a special case of
   inheritance.

   Jack confirmed the two gathering rules: `scratch/` stays recursive;
   `wave/` is top-level only, and reads every top-level `.md`, memory
   included. One gatherer therefore replaces two. Today `MEMORY.md` is
   excluded from `gather_wave_docs` and read by `gather_wave_memory_from`,
   which can use a different checkout than the Wave file. The single read uses the Run's own
   checkout. Memory is a regular file in that read. Jack Heart,
   2026-09-28: a little hinting at the canonical way, and otherwise as much
   a regular citizen as possible. Proposed form, not yet confirmed: the
   separate memory block, its owned/inherited labels and its long
   instructions are removed; one short note rides with the Wave files:

   ```
   Wave files, root first. MEMORY.md holds durable learnings; GOAL.md holds
   the objective. Curate the MEMORY.md of your own Wave,
   wave/infrastructure/release/. Files above it are context.
   ```

   Curation guidance (sections, absolute dates, what belongs elsewhere)
   moves to `update-wave` and `record-learnings`, the skills that exercise
   it.

   Including the parent's Wave file is new: today no ancestor's Wave file is
   read. Whether `update-wave` or chapter review at a parent may read children
   on explicit request was not discussed.
2. **Navigation.** Surfaces may nest the child under the parent. LOO-330 owns
   the presentation.

*Open:* whether anything else crosses. Candidates Jack has not ruled on:
Home placement, budget, chapter review of the parent reading child outcomes.

### Data model

```
waves
  repo        TEXT     -- open: path today; differs per Home
  name        TEXT     -- identity; the slash path under wave/
  created_at, retired_at, retirement_reason
  PRIMARY KEY (repo, name)
  -- dropped: id, parent_wave_id, superseded_by_wave_id, promoted_at

projects
  wave        -- references the Wave by name; changes when a Project moves

tasks
  project_id  -- unchanged
  wave        -- the Wave's name; must equal the Project's wave
```

No scope table, no placement column, no address ledger.

The Task's `wave` is a denormalization. Per the data-model rule recorded in
Infrastructure memory on 2026-09-26, it has a validator: the constructor
fills it from the Project and refuses a mismatch. The same rule already plans
`runs.task_id ⇒ wave` for LOO-298.

| Concept | Identity | Owner of truth |
|---|---|---|
| Wave | its name | the directory and its Wave file; the registry row is a local read model |
| Parent of a Wave | nearest name prefix that is a Wave | computed |
| Task's Wave | `tasks.wave` | the Task, validated against its Project |
| Provider identity | Linear Initiative UUID in the Wave file's frontmatter | Linear |

### Invariants

1. **One hierarchy.** `B` is a child of `A` exactly when `A`'s name is a strict
   path prefix of `B`'s, both are Waves, and no Wave lies between.
2. **Hierarchy never crosses a repository.**
3. **A name is a Wave.** Two Homes that see `infrastructure/release` in the
   same repository mean the same Wave with no id exchange.
4. **A Task has one Wave, equal to its Project's.** Task id, worktree, branch, PR, invocation and worker claim are
   untouched by a rename, because none of them contains the Wave name.
5. **Written text keeps the name it was written with.** PR bodies, Linear
   comments and evidence files are never rewritten. Whether Run rows keep
   the old name is open; see Rename.
6. **A reused name is the same Wave.** History recorded under a name belongs to
   whatever holds the name. Jack Heart confirmed this as correct
   behavior on 2026-09-28.
7. **Wave files come from the Run's own checkout.** There is no single Wave
   memory; it is whatever the checkout for this Task holds. A memory edit is
   an ordinary tracked change: visible to the Task's next step, landed with
   its PR, refreshed by `lf rebase`. Nothing reads `wave/` from the main
   checkout on a Task's behalf. Jack Heart, 2026-09-28.
8. **No surface requires main's copy.** Jack Heart, 2026-09-28: avoid product
   experiences that depend on the main checkout's `wave/`. A command or view
   reads the checkout it was invoked in or the checkout of the Task it shows.
   *Open:* what Wave-level surfaces with no Task read, listed below.

### Release, concretely

Release becomes reachable by becoming a Wave: `wave/infrastructure/release/`
gains a Wave file, a Linear Initiative and a first chapter Project. Its memory
then rides every Run bound to it, after Infrastructure's. The `release-run`
cron moves from Infrastructure's Wave file to release's, which gives release
Runs their subject. LOO-331 found seven release Runs with none.

Jack Heart approved this on 2026-09-28: making release a real Wave is how the
child Wave setup is prototyped. Release is the first subwave, built and used
for real, and what it teaches shapes the rest.

The split itself writes to Linear and moves a live schedule, so it runs from
the installed `lf` after the code lands through ordinary delivery. The branch
proves the same steps in a disposable Home first. *Open:* release's objective
text, which Jack authors or approves.

A directory with only `MEMORY.md` is not a Wave and is not addressable. Until
release is a Wave, its memory file is an ordinary document.

### Rename

Renaming a Wave updates the Wave name recorded on its Project. Jack Heart,
2026-09-28: Projects are relatively ephemeral, but keeping one alive is often
the easiest transfer, and a rename may need only that field updated.

To call `infrastructure/release` `infrastructure/delivery`:

1. Move the directory in Git. The Wave file, memory and Initiative id travel
   with it.
2. Update the name in the store. The Project keeps its id, KRs, metric
   targets and Tasks. The Project and each of its Tasks now say
   `infrastructure/delivery`.

Task ids, worktrees, branches, PRs, invocations and claims are untouched.

*Open: what Run history does on rename.* On LOO-298 a Run row references its
Wave, so the row must point somewhere after a rename.

| | History stays | History follows |
|---|---|---|
| Mechanism | the old name's row remains, retired; Project and Tasks point at the new row | one cascading update of the name |
| `lf runs --wave infrastructure/release` | Runs before the rename | nothing |
| `lf runs --wave infrastructure/delivery` | Runs since the rename | every Run |
| Matches "no surviving renames" | yes | no: the Wave survives under a new name |
| Matches "a reused name is the same Wave" | yes: recreating the name un-retires the row | yes |

*Open:* whether `lf wave relocate` remains as the command for these two
steps, reduced to them.

### Moving work into a Wave that has a Project

Not designed. Jack's statement about Projects moving concerned rename. A Wave
keeps exactly one current Project. Folding one Wave into another uses the
existing chapter transfer of started Tasks, and no new operation is built for
it until the release prototype shows a need.

## What the code does today

Source at `632b67ec7`, paths below `rust/loopflow/src/`. "Verified" means read
directly. "Reported" means found by a delegated search.

### Identity and hierarchy

| Fact | Evidence | Status |
|---|---|---|
| `waves.id` is a random UUID minted by `WaveId::new()` the first time a Home ensures a row | `controller/wave/registry.rs:43-64` | Verified |
| A remote Home adopts the origin's id, and errors when its local id differs | `registry.rs:69-92`, caller `lf/commands/home.rs:193` | Verified |
| `wave_id` appears 570 times in Rust source and in 16 Swift files | grep, 2026-09-28 | Verified |
| `projects.wave_id` references `waves(id)`; `tasks` has `project_id` and no Wave column | `store/migrations/0.11.031_durable_input_spine.sql:7-20` | Verified |
| Metric instruments, placements, epochs and chapters also reference `waves(id)` | migrations | Verified, list incomplete |
| `(repo, name)` is unique among unretired rows only | `store/migrations/0.12.13.001_release.sql:228-261` | Verified |
| `waves.repo` is a path, and the code notes a remote Home may observe a different one | `work/wave/mod.rs:79`, `registry.rs:66-68` | Verified |
| A Wave name is the whole slash path; `.`, `..`, empty and backslash are rejected | `work/wave/mod.rs:33-44` | Verified |
| Discovery is recursive and requires `GOAL.md` | `ops/pm.rs:2527-2556` | Verified |
| `parent_wave_id` has no production writer; all six installed Waves have it null | grep; `lf ls --json` 2026-09-28 | Verified |
| Relocation updates `waves.repo` and `waves.name`, keeps the id, and has no cron handling | `store/sqlite.rs:1870-1877`, `controller/wave/relocate.rs` | Verified |
| Task worktrees and branches derive from the Task slug | this worktree | Verified |
| Cron labels, logs and receipts derive from the name, `/` replaced by `.` | `ops/cron.rs` | Reported |
| Run subjects are the strings `wave:<name>` | `ops/run.rs:201, 227` | Reported |

### What a prompt includes from `wave/`

| Content | Included | Evidence |
|---|---|---|
| `wave/<name>/*.md` of the bound Wave, top level only, `README.md` first | yes, whole | `engine/prompt.rs:455-503`, verified |
| `wave/<name>/MEMORY.md` of the bound Wave | yes, whole, no size cap | `work/wave/context.rs:355-434`, verified |
| Ancestors' memory | only through `parent_wave_id`, so never in practice | same |
| Ancestors' Wave file | no | `engine/prompt.rs` |
| Subdirectories of the bound Wave | no | same |
| Other Waves | no | same |
| Anything, for a Run bound to no Wave | no | reported |

Sizes in this checkout, 2026-09-28, counted with tiktoken `o200k_base`. That
is not the tokenizer of the Claude or Codex model a Run uses, so these are
approximations, and Claude counts usually run higher.

| File | Tokens |
|---|---|
| `infrastructure/MEMORY.md` | 18,400 |
| `product/MEMORY.md` | 12,200 |
| `intelligence/MEMORY.md` | 4,800 |
| `growth/MEMORY.md` | 2,800 |
| `infrastructure/release/MEMORY.md` | 1,100 |
| four `GOAL.md` files | 1,700 |
| whole `wave/` tree | 41,000 |

Jack raised whether a prompt should include more of `wave/` than it does, and
answered: ancestors' Wave files and memory, no children, no siblings. The
target differs from today in two rows: ancestors' memory becomes real, and
ancestors' Wave files are added.

Launch paths and the checkout each reads from are in the kickoff draft at
`0381a195f` and are unchanged by this revision.

### Surfaces with no Task checkout

*Open.* These act on a Wave with no Task, so "the Task's checkout" does not
name a checkout for them. None was re-read from source in this session.

| Surface | Reads from `wave/` |
|---|---|
| Wave discovery and `lf wave list` | which directories are Waves |
| Cron installation | schedules in the Wave file |
| finite `wave/operate` Runs | Wave file and memory |
| Linear connection | the Initiative id in frontmatter |
| The app's Wave page | Wave file and memory |

Wave chat left this list with LOO-298. From a terminal the invoking checkout
answers it. Cron runs in the directory recorded at sync. The app has no
invoking checkout.

## The demo

In a disposable Home and repository fixture:

```
$ lf wave list
infrastructure
infrastructure/release

$ lf task create --wave infrastructure/release "Inspect failed checks while waiting"
LOO-400  infrastructure/release

$ lf task run LOO-400     # prompt carries infrastructure memory, then release memory

$ lf task status LOO-400 --json | jq .wave
"infrastructure/release"

$ lf wave list --json | grep -c '[0-9a-f]\{8\}-[0-9a-f]\{4\}-'
0
```

## Migration and preservation counterexamples

Each is a case the build must prove, stated as what would go wrong.

1. **Installed store with UUID ids.** Every table referencing `waves(id)` holds
   UUIDs on released installations. The forward migration rewrites them to
   names against a populated historical fixture. A reference left as a UUID is
   an orphan that no query finds.
2. **Retired row sharing a live name.** Uniqueness today covers unretired rows
   only, so a store may hold a retired `release` and a live `release` with
   different ids. With the name as key they collide. The migration reports such
   pairs and states which history the name keeps.
3. **Historical Run manifests carrying a UUID.** `lf runs --wave infrastructure`
   must still find Runs whose manifest or `LF_WAVE_ID` recorded the old UUID.
   Manifests are not rewritten, so the migration retains a read-only
   id-to-name mapping for lookup.
4. **Two Homes.** A remote Home that adopted the origin's id must agree on the
   Wave by name after migration, with `ensure_wave_row_with_id` deleted.
5. **Repository key.** `waves.repo` is a path and differs per Home. Two Homes
   must not treat one Wave as two because their checkouts live at different
   paths, and two repositories on one Home may each have `product`.
6. **Project moved while a worker runs.** The Task's claim, invocation id,
   cursor and review Session token are byte-identical afterward. Runs started
   before the move say the old Wave; Runs after say the new one.
7. **Rename with a live Project.** After the name update the Project's KRs,
   metric targets and chapter history read the same, and no second Project
   exists for the Wave.
8. **Non-null `parent_wave_id`.** A row links `release` to `platform` with
   sibling paths, as an existing unit test does. The migration reports such
   rows before dropping the column and renames nothing.
9. **Cron after rename.** The old name's schedule stops with its Wave file.
   Doctor must not report the retired Wave's next interval as a miss, and the
   old launchd label must not keep firing.
10. **Flattened name collision.** `infra/release` and `infra.release` must not
    share a cron label, log file or receipt name.
11. **Branch edits a memory file that main moved.** After `lf rebase` the edit
    lands in the new file, with no resurrected directory at the old name.
12. **Swift and DTO mirrors.** Every `--json` field carrying a Wave id changes
    type and meaning. Rust, Swift and the fixtures under `tests/fixtures/dto/`
    change together.

## Alternatives considered

| Approach | Why not |
|---|---|
| Keep the UUID and add path ancestry (kickoff) | Two identifiers for one thing. Needed a ledger and time-indexed resolution to make names durable. Jack rejected Wave ids. |
| Held scopes: memory-only directories addressable without being Waves (kickoff) | A second kind of thing beside a Wave, with placement stored on Tasks. Jack's direction is that subwaves are Waves. |
| `wave/moves.jsonl` address history (kickoff) | Exists only to survive renames. |
| `tasks.scope` placement column (kickoff) | Placement separate from ownership existed for held scopes, which are gone. |
| Derive a Task's Wave through its Project only | Saves updating Tasks on rename. Renames are rare and reading a Task's Wave is constant. Jack considered and rejected it. |
| Keep `parent_wave_id`, validated against the path | Two hierarchies and a validator forever. |

## The Wave file

*Open.* Jack raised `wave/<name>/<NAME>.md` with some structure, and noted
`GOAL.md` is also simple. For a subwave that would be
`wave/infrastructure/release/RELEASE.md`.

| | `GOAL.md` | `<NAME>.md` |
|---|---|---|
| Discovery rule | fixed filename | filename derived from the directory |
| In an editor or search | four files with one name | each file names its Wave |
| Rename | directory move | directory move and file rename |
| Cost to adopt | none | discovery, docs, skills and four files change |

The identity model does not depend on the answer. Whether the structured file
would also absorb `MEMORY.md` was not discussed.

## Ownership boundaries across the three Tasks

| Question | Owner |
|---|---|
| Wave identity, registry schema, hierarchy, rename, Project moves | LOO-329 |
| How cron and direct Runs acquire a Wave subject | LOO-329 |
| Address grammar, relative and shorthand input, navigation | LOO-330 |
| What a prompt includes from `wave/`, order, precedence, size | LOO-331 |
| Whether a parent's Runs see child memory | Jack Heart answered no on 2026-09-28; both companion drafts proposed a child index and need the correction |

## Scope

- In scope: name as identity; registry migration; computed ancestry; Project
  moves; what rename means; the definition of a subwave.
- In scope, added in review: making `infrastructure/release` a Wave as the
  prototype, including its Wave file, Initiative, first chapter Project and
  the `release-run` cron.
- Out of scope: address grammar and presentation; any Wave budget mechanism;
  cross-repository hierarchy; other subwaves; splitting the rest of
  Infrastructure's memory; running a branch binary against the installed
  Home.

## Forbidden outcomes

- A Wave id kept beside the name, as a cache or for compatibility.
- `parent_wave_id` kept beside path ancestry.
- A Task whose Wave differs from its Project's.
- A historical record rewritten to a new name.
- A rename that restarts, reclaims or re-registers a Task, worktree or PR.
- Running a branch binary against the installed Home to prove any of this.

## Internal slices

1. **The path read.** One gatherer: every top-level `.md` of the Wave's
   directory and each ancestor's, root first, from the Run's checkout, with
   the short note. `gather_wave_memory_from` and its registry walk are
   deleted. Proof: a fixture Run at `infrastructure/release` carries four
   files in order and nothing from a sibling.
2. **Release becomes a Wave.** Wave file, Initiative, chapter Project, cron
   moved. Uses slice 1 and today's registry unchanged: a slash-qualified name
   with `GOAL.md` is already discovered as a Wave. Performed on the real
   repository after slice 1 is installed.
3. **Drop `parent_wave_id`.** Its remaining readers answer from the name.
   Counterexample 8.
4. **Name as identity.** The migration, the `WaveId` type, deletion of
   `ensure_wave_row_with_id` and supersession, DTO mirrors. Counterexamples
   1 to 5 and 12. This is the deep change and touches released installations.
5. **Project moves and rename.** Counterexamples 6, 7, 9, 10, 11.

Slices 1 and 2 give a working, used subwave before any schema change. What
release shows in use can still change slices 3 to 5.

## Done when

For this Task: Jack Heart completes review-design, LOO-330 and LOO-331 are
corrected to this vocabulary, and a real Run at `infrastructure/release`
carries Infrastructure's files and then release's.

For the build, in disposable Homes with inherited `LF_*` authority removed:
the demo runs as written, and each counterexample has one behavioral proof.

## Slice ledger

- 2026-09-28: Kickoff source audit at `632b67ec7`. No test run.
- 2026-09-28: Rebased onto LOO-298 at `d07e56933` by moving the scratch notes;
  `lf rebase --plan` would have replayed three main commits across 203 files.
  This branch's questions moved to `subwave-questions.md` because LOO-298 owns
  `scratch/questions.md`. Schema drafts, memory chain, relocation, cron and
  the architecture reference re-read. No test run.
- 2026-09-28: Review-design with Jack Heart. Wave ids, held scopes, the
  address ledger and Task placement removed at his direction. Id minting,
  schema references and prompt inclusion re-read from source; `wave/` sizes
  measured. No test run. No message sent to the LOO-330 or LOO-331 workers.
