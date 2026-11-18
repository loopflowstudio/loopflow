# LOO-978: served reads reuse resolved settings and the skill catalog

## Outcome

`lf server run`, Desktop and every `--subscribe` reader stop re-validating
configuration on each read. Resolved settings and the skill catalog are
computed once per change to the files they came from; every reading until then
gets the same shared value. Nothing changes in what any read returns, and bare
CLI invocations (`lf config settings --explain`, launches, `--setting`) compute
inline exactly as today.

Seen 2026-11-18 on lf 0.16.0+8abce99e8 with about 40 agents writing: a 3 s
`sample` of the server put ~1,700 busy samples in `validate_value` and ~95 in
`SkillCatalog::collect`/`load`, against ~2,260 for Session projection reads.

Why `validate_value` is that large: `filter_file_preferences` calls it once per
leaf of every file layer, and each call clones the whole defaults tree and
deserializes a whole `Config` (`settings.rs:219-222`). `resolve_layers` runs
that for every layer on every call, and a reading calls it twice per Task
(`resolve_task_agent`) and once per named skill per Wave (`read_wave_setup`).

## Design: derived values in `observe`

`observe` already is the one notion of "the files changed": a served reading
runs inside `Cache::capture`, each `observe::read_to_string` / `read_dir` /
`is_file` returns the cached answer and records `(entry, revision)` in the
reading's `Dependencies`; the background refresh bumps a revision when an
answer differs (`rust/loopflow-execution/src/observe.rs`). A subscription reads
again when `Dependencies::changed()`. The only production reading is
`read_stream.rs:610`; nothing else has a cache.

Add the next layer on the same mechanism: a value computed from observed facts
is kept with the dependencies its computation read.

### The key is a closed list, like `Fact`

```rust
/// A value computed from observed facts. Each variant is the whole identity
/// of one computation: everything it reads that is not an observed fact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Derived {
    Settings { checkout: Option<PathBuf>, wave: Option<String>, outside: Outside },
    WaveSetup { checkout: PathBuf, wave: String, outside: Outside },
    SkillCatalog { repo: Option<PathBuf>, home: Option<PathBuf>,
                   claude: Option<PathBuf>, codex: Option<PathBuf> },
}

/// `compute`'s result, shared until a fact it observed changes.
pub fn derived<T: Send + Sync + 'static>(key: Derived, compute: impl FnOnce() -> T) -> Arc<T>
```

A closed enum and not a generic `K`: every cached computation and what
identifies it is one list in `observe.rs`, the way `Fact` is the list of what
is observed. A generic key needs type-erased hashing and scatters that list
across call sites. Passed over.

- **No reading active** (bare CLI): `Arc::new(compute())`. No cache exists.
- **Reading active, hit**: the slot's dependencies are unchanged, so return the
  shared value and make the current reading depend on the slot's dependencies.
- **Miss or changed**: run `compute` under a child reading (same `Cache`, fresh
  `Dependencies`), store value and dependencies, and make the parent depend on
  them.
- The slot is locked while computing, so four detail workers asking for one
  repository's settings compute once. A `compute` that panics leaves the slot
  empty; the next caller computes. It never poisons later readings.
- Slots live in `Cache` and leave by the rule fact entries already use (unused
  for 300 s). A slot's dependencies hold their entries (`strong_count > 1`), so
  the existing refresh keeps checking those files. No watcher, no timer, no
  size knob.
- Staleness is what served reads already have: the Local refresh interval (1 s).

### Dependencies nest; they are not copied

```rust
pub struct Dependencies(Arc<Mutex<Read>>);
struct Read {
    facts: HashMap<Fact, (SharedEntry, u64)>,
    derived: HashMap<Derived, Dependencies>,   // the slot's own, by reference
}
// changed() = any fact's revision moved || any derived.changed()
```

A reading that uses a derived value holds that value's `Dependencies` by
reference. A hit is one map insert, whatever the value read to be computed; the
catalog reads several hundred facts, and copying them into every reading that
asks would leave a per-read cost proportional to the catalog. A slot that is
computed again gets a fresh `Dependencies`; a reading still holding the old one
sees it as changed, which is the wake-up. Revisions are the ones recorded when
the slot was computed, so a change between compute and use stays visible.

### What a computation may read

Observed facts, and its key. Nothing else: not the environment, not a
thread-local, not the store. An unobserved input makes a slot stale until one
of its files changes; today the next store revision picks such inputs up.

For settings that rule is carried by the code, not by convention.
`resolve_layers` reads three things from outside today: `environment()`,
`invocation_values()` and `default_agent()`. They move into one value:

```rust
/// What settings resolution reads that is not a file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Outside { environment: String, invocation: String,
                     sole_harness: Option<&'static str>, home: PathBuf,
                     remote: Option<String> }
pub fn outside() -> Result<Outside, LoadError>;      // reads them, once per call
fn resolve_with(layers, skill, &Outside) -> Result<ResolvedSettings, LoadError>;
pub fn resolve_layers(layers, skill) = resolve_with(layers, skill, &outside()?)
```

`Outside` is both the key and the argument, so the key cannot drift from what
the computation read. `resolve_layers` keeps its signature for its other
callers. Read per call as today; nothing in the sample points at it.

- `--setting` overrides: when `outside.invocation` is non-null the settings
  are computed inline and never enter a slot. A choice: keying on it would also
  be correct, and was passed over so one command's override cannot occupy a
  shared slot.
- Wave named by ID (`files::wave_name` walks `.lf/wave` with `ignore`): resolve
  the name before `derived`; the key holds the name.
- Main checkout of a worktree (`std::fs` in `git::worktree_root`, under
  `files::private_path`): fixed for a checkout path; deleting the checkout
  changes `.lf/config.yaml`, which is a dependency.
- `fs::canonicalize` on skill dirs (`catalog.rs`): symlink retargets show as
  changed `Metadata`/`File`/`Directory` answers for the lexical path.
- `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `provider_overrides`: resolved to paths
  before `derived`; they are the catalog key's `claude` and `codex`.
- Store rows (Wave rows, placement scope chain): read outside any computation.

Everything else on these paths already goes through `observe`
(`load_yaml_file`, `files::read_text`, `read_wave_file`, catalog `collect`,
`is_generated`, `SkillSource::read`).

Launches and mutations are not affected: only the subscription reading has a
cache, so a stale value could reach a display, never a launched agent.

### What is derived: three values

1. **Settings for a checkout and Wave.** `config::resolved(repo, wave) ->
   Result<Arc<ResolvedSettings>, LoadError>` is `config_layers` +
   `resolve_with(_, None, outside)`. `load_wave_config`, `load_config`,
   `load_global_config`, `scope_graph_default` and the other `Config`-returning
   helpers read from it; layers come from `config.setting_layers`.
   `resolve_task_agent` with no skill reads the agent from it: today that is
   two full resolves per Task per reading.
   `LoadError` derives `Clone` (every variant holds a `String`), so the slot
   holds the `Result` itself and both paths return the same variant and text.
   `task_configuration_refusal` and `lf config` match
   `LoadError::AgentSelection`; a slot that kept only a message would break
   them.
2. **A Wave's settings half of `WaveSetup`** (`wave/setup.rs:44-57`: graph,
   agent and one resolve plus one skill load per skill named in any layer).
   `machine()` reads the store and stays outside.
3. **The skill catalog.** `SkillCatalog::load(repo, home, overrides)` returns
   the shared `Arc<SkillCatalog>`; the walk and each `is_generated` front
   matter parse run once per change under `.lf/skills`, `.claude`, `.agents`,
   `.codex`. `load_skill` on top stays per call unless the sample shows it.

Machine placement (`store/placement.rs:49`) resolves layers holding only
`machine`, a few leaves each. It is not derived unless the sample after the
catalog slice still shows `validate_value` under `resolved_placement`; then it
is a fourth variant keyed by `(home, scope chain)`.

File warnings (`reader::warn`, stderr) are printed when a value is computed, so
the server says each once per file change per checkout, not once per read.

## What goes

Two earlier per-reading caches for this same cost, superseded by (1):

- `config::PersonalSettings`, `load_wave_config_with`, `config_layers_with`
  and the `personal` parameter threaded through `resolve_task_agent`,
  `task_configuration_refusal`, `task_process_refusal_after`
  (`ops/task.rs:2274-2318`), `ops/task/snapshot.rs:32`, `session.rs:15`.
  `PersonalSettings` flattened a personal file's error to
  `InvalidGraph(message)`; without it the error keeps its own variant. Check
  the text any test pins.
- In `rust/loopflow/src/lf/commands/waves.rs`: the `personal` map
  (`:1876-1884`, filled at `:1953-1963`) and the `agents` memo keyed by
  serialized layers (`:1889-1892`, `:1998-2027`).

Must survive unchanged: `lf config settings --explain`, `--setting` overrides,
`configuration_changes_reach_an_existing_subscription_without_store_writes`,
`new_and_edited_skill_overrides_reach_an_existing_subscription`,
`wave_setup_tests::`, the settings unit tests in `config/settings.rs`.

## Acceptance at gate

`cargo test -p loopflow-execution --lib read_stream::tests::` with one new test
beside the two "changes reach" tests: a pulled subscription whose read calls
`load_wave_config` and `SkillCatalog::load`; three checkpoints force three
readings and settings resolve once and the catalog collects once (a
`#[cfg(test)]` count of computations per key in `observe`); an edit to
`.lf/config.yaml` resolves once more and the frame shows the new value; a
malformed file yields the same answer as today; an invocation override set
around a reading changes that reading's answer and leaves the next one's
untouched; two homes read through one `Cache` get their own global settings.
Break `derived` to always compute and the count fails; break dependency
nesting and the existing "changes reach" tests hang.

`cargo test -p loopflow --test integration wave_setup_tests::` passes.

## Demo

On a Machine under the same kind of load, with this branch's `lf` as the
server: `sample $(pgrep -f 'lf server run') 3 | grep -c validate_value` before
and after, and the CPU column of `ps`. Before: ~1,700 samples. After: none
outside the second after a config edit. The numbers go in the PR body.
Installing a branch build over a server with live Sessions is a person's call;
the headless stand-in is slice 1's timing.

## Slices

1. **`observe::derived` and settings (1).** `Derived`, nested `Dependencies`
   and the slot, with unit tests in `observe.rs` (hit, invalidation by a
   changed fact, a nested value waking the outer reading, bare call computes
   inline, concurrent callers compute once, a panic leaves the slot usable).
   Then `Outside`, `resolve_with` and `config::resolved` under
   `load_wave_config`. Timing in the acceptance test's shape: 1,000 forced
   readings of `load_wave_config` over three layers, before and after, and how
   much of one cold resolve is `validate_value`. This is the slice that could
   prove the plan wrong: if a "changes reach" case fails, an input is
   unobserved and belongs in `Outside` or behind `observe`.
2. **Task agents and Wave setup (1 for Tasks, 2)**, deleting
   `PersonalSettings`, the `_with` variants and both `waves.rs` maps in the
   same change.
3. **Skill catalog (3)**, then a `sample` of a locally run server reading a
   seeded store to see what of `validate_value` and `collect` is left. Derive
   placement only if it shows. The `observe` line in `AGENTS.md` gains derived
   values.

Checks: slice tests run locally by the implementing step; clippy and the full
matrix at gate/CI; the live `sample` by a person.

## Larger than this Task

Left on `scratch/concept-review.html`, not built here:

- Validity is a fact about a file, and `resolve_layers` works it out again for
  every resolution and every skill. A `Config` carries its raw layers
  (`setting_layers`) so that a resolved value can be resolved again.
- `validate_value` deserializes a whole `Config` per leaf. Bare `lf` commands
  pay that on every invocation and nothing here changes it.
- A warning about a bad setting, found by a served read, goes to the server's
  stderr. The person editing the file in front of Desktop sees nothing.

## Docs written ahead of the code

- `rust/loopflow/src/server/README.md`, after "external observations do not
  share that transaction": what a subscriber sees when a settings or skill
  file changes, then `observe::derived` and the rule for what a computation
  may read. The code must match it: about a second to a new reading, one
  warning per edit, an override never enters the shared value.
- `AGENTS.md`, the `observe` line under `*.rs`: derived values. Written now,
  so slice 3 no longer adds it.

No page under `docs/` changes: nothing a read returns changes.
