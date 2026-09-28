# Subwaves: a Wave, its parent, and its address (Infrastructure · LOO-329)

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
| LOO-298's `scratch/` holds about 705 KB of Markdown | Removed from this branch at Jack Heart's direction, except `data-model-one-table-per.md`, the base's design. The files remain on LOO-298's branch. |

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

**A Wave has an id, its own name, and a parent. Its address is the names from
the root joined by `/`. The address is how people and files refer to a Wave;
the id is what records refer to.**

Jack Heart reached this in review on 2026-09-28 after first choosing the name
as the identity. The requirement that reversed it, in his words: "release
shoudlnt need to switch to infra/release if infrastructure changes to infra".
His conclusion: "i guess we need raw parent id, with fast input / syntax or
something". The direction is stated tentatively.

| Decision | Source |
|---|---|
| Wave UUIDs are kept. | Jack Heart, 2026-09-28, reversing his earlier "wave id = wave name" |
| A Wave stores its parent's id. | Jack Heart, same session |
| The slash path is input syntax for reaching a Wave. | Jack Heart, same session |
| Renaming a parent does not change its children. | Jack Heart, same session |
| Rename does not change history. | Jack Heart, same session |
| Context flows down: every top-level `.md` of the Wave's directory and each ancestor's, memory included, from the Run's checkout. No children, no siblings. `scratch/` stays recursive. | Jack Heart, same session |
| Memory is a regular file with light hinting. | Jack Heart, same session |
| No surface requires main's copy of `wave/`. | Jack Heart, same session |
| Release becomes a real Wave, as the prototype. | Jack Heart, same session |

### Data model

```
waves
  id              TEXT PK     identity; never changes
  name            TEXT        this Wave's own segment: "release"
  parent_wave_id  TEXT NULL   references waves(id); NULL at the root
  repo            TEXT
  created_at, retired_at, retirement_reason
  UNIQUE (repo, parent_wave_id, name) among unretired rows
```

Today `name` holds the whole path and is unique per repository. Here it holds
one segment and is unique among siblings. The address is computed by walking
parents and is stored on no row.

| | Example |
|---|---|
| id | `6155f18a-…` for infrastructure; a new one for release |
| name | `infrastructure`; `release` |
| parent | none; infrastructure's id |
| address | `infrastructure`; `infrastructure/release` |
| files | `wave/infrastructure/`; `wave/infrastructure/release/` |

Everything else references a Wave by id, as LOO-298 already does: Projects,
Runs, agent Sessions, Flow Sessions, metric instruments, placements.

### Two hierarchies, one author

The directory tree and `parent_wave_id` both express parentage. The kickoff
audit found the failure this invites: nothing wrote the column, so nesting a
directory conferred nothing.

Proposed, not confirmed: **the directory tree is authored; the column records
it.** Discovery finds `wave/infrastructure/release/GOAL.md`, takes the nearest
enclosing Wave directory as the parent, and writes the parent's id. A
directory moved in Git is re-recorded at the next discovery. Nothing refuses
because the two differ.

The context read does not consult the column. It reads directories on the path
in the Run's checkout, so it works in a fresh clone with no rows.

*Open:* whether a parent may ever differ from the enclosing directory. If it
may not, the column is a record of the tree and needs only the discovery
writer. If it may, the column is authored and the tree is a convention.

### What a subwave shares with its parent

A subwave has its own objective, Project, Tasks, Linear Initiative, crons and
memory file. Two things cross the boundary:

1. **Context flows down only.** A Run at `infrastructure/release` reads:

   ```
   read     wave/infrastructure/*.md
   read     wave/infrastructure/release/*.md
   skipped  wave/infrastructure/auth/          sibling
   skipped  wave/product/, wave/growth/, …     parent's siblings
   ```

   | Run at | Approx. tokens from `wave/` |
   |---|---|
   | `infrastructure` | 18,900 |
   | `infrastructure/release` | 20,000 plus release's Wave file |

   One gatherer replaces two. `MEMORY.md` stops being excluded from the Wave
   read, and `gather_wave_memory_from` with its registry walk is deleted. One
   short note rides with the files. Its wording is proposed, not confirmed:

   ```
   Wave files, root first. MEMORY.md holds durable learnings; GOAL.md holds
   the objective. Curate the MEMORY.md of your own Wave,
   wave/infrastructure/release/. Files above it are context.
   ```

   Curation guidance moves to `update-wave` and `record-learnings`.
2. **Navigation.** Surfaces may nest the child under the parent. LOO-330 owns
   the presentation.

*Open:* whether `update-wave` or chapter review at a parent may read children
on explicit request, and whether Home placement or budget crosses.

### Addresses as input

`--wave infrastructure/release` resolves by walking names from the root.
LOO-330 owns the grammar, including whether a bare `release` resolves when
only one Wave has that name. Infrastructure persists ids and resolves
addresses; it stores no address.

### Invariants

1. **Identity never changes.** Rename and reparent leave the Wave id, and
   every row that references it, untouched.
2. **Rename touches one row.** Renaming `infrastructure` to `infra` updates
   that Wave's name and moves its directory. Release's row is not written.
   Release's address becomes `infra/release` because addresses are computed.
3. **Reparent touches one row.** It updates `parent_wave_id` and moves the
   directory.
4. **History does not change.** Run, Session and Flow Session rows keep the
   Wave id they were recorded with. They display the Wave's current address.
5. **Written text keeps the address it was written with.** PR bodies, Linear
   comments and evidence files are never rewritten.
6. **A reused name is a new Wave.** A Wave created at a retired Wave's address
   gets a new id and none of the old history. This reverses what Jack
   accepted under name-as-identity, and follows from keeping ids.
7. **Hierarchy never crosses a repository.**
8. **Wave files come from the Run's own checkout.** There is no single Wave
   memory. A memory edit is an ordinary tracked change.
9. **No surface requires main's copy of `wave/`.**

### A Task's Wave

*Open.* Jack first proposed `task.wave = task.project.wave`, then judged the
join the wrong tradeoff because renames are rare, then said he is unsure of
the right API. With ids kept, rename no longer touches Tasks either way, so
the choice is only about reads. LOO-298's pattern is a stored ancestor filled
by the constructor and validated by trigger, as `runs.wave_id` is.

### Release, concretely

Release becomes a Wave: `wave/infrastructure/release/` gains a Wave file, a
Linear Initiative and a Project carrying the shared chapter name. Discovery
creates its row with name `release` and Infrastructure's id as parent. The
`release-run` cron moves to release's Wave file.

The split writes to Linear and moves a live schedule, so it runs from the
installed `lf` after the code lands. The branch proves the same steps in a
disposable Home first.

On this base `wave/infrastructure/release/` does not exist. LOO-298 branched
before main gained release's memory and `.lf/flows/release-run.yaml` in
[#1311](https://github.com/loopflowstudio/loopflow/pull/1311). The prototype
needs LOO-298 caught up to main, or those two files carried over.

**Release's Wave file, drafted from existing copy.** Jack Heart, 2026-09-28:
"just read whatever we have in exsiting copy for infra + release". Every
sentence below is taken from Infrastructure's `GOAL.md`, Infrastructure
memory's Model section, or release memory, with only the subject changed. Not
written to `wave/`.

```markdown
---
crons:
- flow: release-run
  schedule: 0 0 10 * * *
pm:
  linear_initiative: <created by the split>
---

## Objective

Loopflow delivers verified releases on the configured release schedule.
Recovery preserves work, recorded decisions and history; failures are
bounded, truthful, and actionable.

## Bounds

- Do not build a generic multi-product deploy platform before a second real
  product proves the shape.
- Release owns the automation spine, not release-content substance: each
  product owns its own changelog and provider-specific agent credentials.

## Cron

- `release-run` -> attempt one patch release. No merged changes is a green
  no-op; an incomplete tagged release resumes from its hosted build.

## Process

Keep the configured release schedule and required verification. An entry
point must preserve recovery ownership. Cron must observe the operation's
result: a successful report of failure is not successful release execution.
```

Infrastructure's Wave file would lose its `release-run` cron entry and its
Cron line for it. Its objective keeps "delivers verified releases" unless Jack
narrows it. One existing phrase no longer fits: `release-run` is described as
running "after telemetry", an ordering between two Waves' schedules.

### Rename and reparent

| | Rename `infrastructure` to `infra` | Move release under `delivery` |
|---|---|---|
| Git | `wave/infrastructure/` moves, children inside it | `wave/infrastructure/release/` moves |
| Store | one row: name | one row: parent |
| Release's row | untouched | parent changes |
| Runs, Projects, Tasks | untouched | untouched |
| Addresses shown | every descendant's, computed | release's and its descendants' |

`lf wave relocate` is left as LOO-298 leaves it. Jack Heart, 2026-09-28:
revisit after LOO-298 is done, stated with "idk". Slice 4 is deferred with
it.

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
| The app's Wave page | Wave file and memory; the desktop UI is not changed for now (Jack Heart, 2026-09-28) |

Wave chat left this list with LOO-298. From a terminal the invoking checkout
answers it. Cron runs in the directory recorded at sync. The app has no
invoking checkout.

### Cron today

Researched at Jack Heart's request, 2026-09-28. Installed jobs and receipts
were read from his machine, read-only; source was read on LOO-298.

| Fact | Evidence |
|---|---|
| Both installed jobs run in the main checkout, `/Users/jack/src/loopflow` | `~/Library/LaunchAgents/loopflow.cron.infrastructure.*.plist` |
| All 69 scheduled receipts record that directory | `lf cron history --wave infrastructure --json` |
| A job runs `lf --wave <wave> --batch <kind> <flow>` with that directory as its working directory | `ops/cron.rs:770-780` |
| `telemetry-daily` is two read-only operations, `doctor` and a scorecard | `.lf/flows/telemetry-daily.yaml` |
| `release-run` starts in that directory and does its work in generated `prepare-…` and `publish-…` worktrees | `engine/builtins/ops/skill/release-run.md:55-62` |
| A scheduled release retry removed two uncommitted memory edits from main | Infrastructure memory, 2026-08-23, LOO-266 |
| The log path uses the Wave name unescaped: `cron.{wave}.{flow}.log` | `ops/cron.rs:40-44`; only the Flow's `/` is replaced |

The existing pattern is: start in main, do effects in generated worktrees.
Reads are what still depend on main's working tree: the Wave files a scheduled
prompt carries are whatever main holds, uncommitted edits included.

Read from source and not run: a Wave named `infrastructure/release` would put
its log at `.lf/logs/cron.infrastructure/release.release-run.log`, inside a
directory nothing creates. The release prototype would meet this first.

## The demo

In a disposable Home and repository fixture:

```
$ lf wave list
infrastructure
infrastructure/release

$ lf task create --wave infrastructure/release "Inspect failed checks while waiting"
$ lf task run LOO-400     # prompt carries infrastructure's files, then release's

$ git mv wave/infrastructure wave/infra && lf wave list
infra
infra/release

$ lf runs --wave infra/release    # the Run from before the rename is listed
```

Release's row has the same id, name and parent before and after.

## Migration and preservation counterexamples

Each is a case the build must prove, stated as what would go wrong.

1. **Parent renamed, child untouched.** After `infrastructure` becomes
   `infra`, release's row is byte-identical and its Runs are listed under
   `infra/release`.
2. **Rename while a worker runs.** The Task's claim, Flow Session, cursor and
   review Session token are byte-identical afterward.
3. **Existing rows hold whole paths as names.** The migration splits a nested
   name into segment and parent. A nested row whose parent has no row gets one
   or is reported; it is never flattened.
4. **Non-null `parent_wave_id` that disagrees with the directory.** A row
   links `release` to `platform` with sibling paths, as an existing unit test
   does. Which wins depends on the open question above.
5. **Fresh clone or second Home.** Files exist and rows do not. Context reads
   work. Two Homes sync a Wave's id when nothing conflicts. Jack Heart,
   2026-09-28: conflict handling comes later. A conflict is reported and
   nothing is overwritten.
6. **Reused address.** A new Wave at a retired Wave's address shows none of
   the old Runs.
7. **Same name under two parents.** `infrastructure/release` and
   `product/release` coexist.
8. **Cron after rename.** Labels, logs and receipts derive from the address.
   Renaming a parent changes every descendant's derived label. Doctor must not
   report a miss and the old launchd label must not keep firing.
9. **Flattened address collision.** `infra/release` and `infra.release` must
   not share a cron label, log file or receipt name.
10. **Branch edits a memory file that main moved.** After `lf rebase` the edit
    lands in the new file, with no resurrected directory at the old address.
11. **Sibling never read.** A Run at `infrastructure/release` carries nothing
    from `wave/infrastructure/auth/`.

## Alternatives considered

| Approach | Why not |
|---|---|
| Name as identity, no UUID | Jack chose it, then withdrew it. Renaming a parent would rename every child, and every record of them. It also reverses LOO-298's written contract and rewrites 551 references. |
| Parentage implicit in a whole-path name | Same objection: the child's stored name contains the parent's. |
| Address ledger and time-indexed resolution (kickoff) | Existed to make names durable. Ids are durable. |
| Held scopes: memory-only directories that are not Waves (kickoff) | A second kind of thing beside a Wave. A subwave is a Wave. |
| Drop `parent_wave_id`, compute ancestry from the path (kickoff) | Works for the context read, and the read still does it. As the stored relation it requires the whole path in the name. |

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
| Wave identity, parent relation, address resolution, rename, reparent | LOO-329 |
| How cron and direct Runs acquire a Wave subject | LOO-329 |
| Address grammar, relative and shorthand input, navigation | LOO-330 |
| What a prompt includes from `wave/`, order, precedence, size | LOO-331 |
| Whether a parent's Runs see child memory | Jack Heart answered no on 2026-09-28; both companion drafts proposed a child index and need the correction |

## Scope

- In scope: the parent relation and who writes it; name as a segment; address
  resolution; the path read; rename and reparent; the definition of a
  subwave.
- In scope, added in review: making `infrastructure/release` a Wave as the
  prototype, including its Wave file, Initiative, first chapter Project and
  the `release-run` cron.
- Out of scope: address grammar and presentation; any Wave budget mechanism;
  cross-repository hierarchy; other subwaves; splitting the rest of
  Infrastructure's memory; running a branch binary against the installed
  Home.

## Forbidden outcomes

- A child row written because its parent was renamed.
- A whole address stored on a Wave row.
- A context read that needs the registry.
- A guard that refuses because the directory tree and the parent column
  differ.
- A sibling's or child's files in a prompt.
- A historical row rewritten on rename.
- Running a branch binary against the installed Home to prove any of this.

## Internal slices

1. **The path read.** One gatherer, from the Run's checkout, with the short
   note. Counterexample 11. No schema change.
2. **Parent recorded, name is a segment.** Discovery writes `parent_wave_id`;
   the migration splits nested names; address resolution walks parents.
   Counterexamples 3, 4, 5, 7.
3. **Release becomes a Wave.** Performed on the real repository after slices
   1 and 2 are installed.
4. **Rename and reparent.** Counterexamples 1, 2, 6, 8, 9, 10.

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
- 2026-09-28: Jack Heart withdrew name-as-identity later in the same review.
  Ids and a stored parent are kept; the name becomes one segment. The
  name-as-identity draft is at commit `2f54a48f7`.
- 2026-09-28: Review-design with Jack Heart. Wave ids, held scopes, the
  address ledger and Task placement removed at his direction. Id minting,
  schema references and prompt inclusion re-read from source; `wave/` sizes
  measured. No test run. No message sent to the LOO-330 or LOO-331 workers.
