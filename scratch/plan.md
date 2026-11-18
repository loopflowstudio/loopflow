# LOO-978: served reads reuse resolved settings and the skill catalog

## Outcome

`lf server run`, Desktop and every `--subscribe` reader stop re-validating
configuration on each read. Resolved settings and the skill catalog are
computed once per change to the files they came from; every reading until then
gets the same shared value. Nothing changes in what any read returns. A bare
CLI invocation (`lf config settings --explain`, launches, `--setting`) keeps
nothing between commands; bare Task details run as one reading for the length
of the call (`observe::reading`), so Tasks sharing a checkout resolve once.

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
    GlobalSettings { outside: Outside },   // added by qa; home is outside.home
    SkillAgents { checkout: PathBuf, wave: String, outside: Outside },
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
pub struct Outside { environment: Value, invocation: Value,   // serde_yaml_ng, Hash + Eq
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
  shared slot. Environment settings are process-wide, so they stay in the key.
- Wave named by ID (`files::wave_name` walks `.lf/wave` with `ignore`): resolve
  the name before `derived`; the key holds the name.
- Main checkout of a worktree (`std::fs` in `git::worktree_root`, under
  `files::private_path`): the settings computation observes the checkout's
  `.git` first, so a worktree that appears later changes its dependencies.
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

### What is derived

1. **Settings for a checkout and Wave.** `config::resolved(repo, wave) ->
   Result<Arc<ResolvedSettings>, LoadError>` is `file_layers` +
   `resolve_with(_, None, outside)`. `load_wave_config` and
   `resolve_task_agent` read from it; layers come from
   `config.setting_layers`. `resolve_task_agent` with a skill still resolves
   per call on top of the shared value; the Wave detail read passes none.
   `LoadError` derives `Clone` (every variant holds a `String`), so the slot
   holds the `Result` itself and both paths return the same variant and text.
   `task_configuration_refusal` and `lf config` match
   `LoadError::AgentSelection`; a slot that kept only a message would break
   them.
2. **A Wave's skill agents** (`work/wave/setup.rs`, `Derived::SkillAgents`):
   one resolve plus one skill load per skill named in any layer. Its
   computation calls `config::resolved_with`, so nesting is used by real
   code. `WaveSetup`'s graph and agent come straight from (1); `machine()`
   reads the store and stays outside. Its skill loads go through
   `SkillCatalog::load`, so they nest on (3) and its key.
3. **The skill catalog.** `SkillCatalog::load(repo, home, overrides)` returns
   the shared `Arc<SkillCatalog>`; the walk and each `is_generated` front
   matter parse run once per change under `.lf/skills`, `.claude`, `.agents`,
   `.codex`. `load_skill` on top stays per call unless the sample shows it.

4. **The global file's settings alone** (`config::placement::global_settings_at`,
   `Derived::GlobalSettings`): Machine names and aliases, asked by ownership,
   placement and Machine reads. Added after qa's sample.

Machine placement (`store/placement.rs:49`) resolves layers holding only
`machine`, a few leaves each. It is not derived: qa's sample put
`resolved_placement` under 10 samples.

File warnings (`reader::warn`, stderr) are printed when a value is computed, so
the server says each once per file change per checkout, not once per read.

## What went

`config::PersonalSettings`, `load_wave_config_with`, `config_layers_with`, the
`personal` parameter through `ops/task.rs`, `ops/task/snapshot.rs` and
`session.rs`, and the `personal` and `agents` maps in
`rust/loopflow/src/lf/commands/waves.rs`. A personal settings file that fails
to load now reports its own variant (`io error: …`) where `PersonalSettings`
wrapped it as `invalid graph: …`; no test pinned the old text.

Must survive unchanged: `lf config settings --explain`, `--setting` overrides,
`configuration_changes_reach_an_existing_subscription_without_store_writes`,
`new_and_edited_skill_overrides_reach_an_existing_subscription`,
`wave_setup_tests::`, the settings unit tests in `config/settings.rs`.

## Acceptance at gate

Built, in `loopflow-execution`:

- `read_stream::tests::readings_share_resolved_settings_until_a_settings_file_changes`:
  a pulled subscription reading `load_wave_config`; four readings resolve
  once, an edit resolves once more and the frame shows it, a file that does
  not parse answers as a bare command does, a `--setting` override changes its
  own reading only, a second `LF_HOME` through the same `Cache` gets its own
  settings.
- `skills::catalog::tests::readings_share_one_catalog_until_a_skill_file_changes`
  and `work::wave::setup::tests::readings_share_a_waves_skill_agents_until_a_settings_file_changes`,
  through `Cache::capture`.
- `observe::tests::`: shared until a fact changes, nested wake-up, four
  workers compute once, a panic leaves the slot to the next caller.

Counts come from a `#[cfg(test)]` list of computed keys on `Cache`. The
catalog reaching a live subscription is still
`new_and_edited_skill_overrides_reach_an_existing_subscription`.

## Demo

On a Machine under the same kind of load, with this branch's `lf` as the
server: `sample $(pgrep -f 'lf server run') 3 | grep -c validate_value` before
and after, and the CPU column of `ps`. Before: ~1,700 samples. After: none
outside the second after a config edit. The numbers go in the PR body.
Installing a branch build over a server with live Sessions is a person's call;
the headless stand-in is slice 1's timing.

## State

Built and committed in three commits: `observe::derived` with nested
`Dependencies`, `Outside`, `resolve_with` and `config::resolved`; Task agents
and Wave skill agents on the shared settings with the per-reading caches
deleted; `SkillCatalog::load` returning the shared `Arc<SkillCatalog>`.

What a run showed (2026-11-18, debug build, 1,000 readings of
`load_wave_config` over three layers of eight leaves): 8.9 s resolving every
time, 22 ms through one `Cache`. Filtering the same three layers 1,000 times
(`filter_file_preferences`, which is `validate_value` per leaf) took 4.4 s:
about half of a cold resolve.

Still paid on every call, as before this change: `outside()` reads the
environment and stats each harness name along `PATH` (`default_agent`), once
per Task per reading in Wave detail; `load_wave_config` clones the shared
`Config`. Neither has been measured.

A reading's first resolve is now once per Task checkout, where the deleted
`waves.rs` memo resolved once per distinct configuration within a reading.
After a server start that is one cold resolve per worktree, then none.

Left, not code: the live `sample` under load (Demo above), by a person. The
`sample` of a locally run server over a seeded store is under "What qa
changed".

Checks run by this step, all passing:

- `cargo test -p loopflow-execution --lib -- observe::tests:: work::wave::setup:: read_stream::tests:: config:: ops::task::tests:: skills:: graph::`
- `cargo test -p loopflow --test integration wave_setup_tests::` and `discovery_tests::`
- `cargo test -p loopflow --lib lf::commands::waves::`
- `cargo clippy --all-targets -- -D warnings`

For gate/CI: the full matrix; `test(performance_tests::)` and
`test(documented_commands::)` were not run here.

## What qa changed (2026-11-18)

A `sample` of a branch server under write load still showed 649 samples in
`resolve_with` against the base's 5,564. Two callers resolved outside the
shared values, and the bare CLI had become slower. All three are fixed:

- `ops::task::scope_graph_default` (each Task's default Graph, once per Task
  per reading) resolved its own layers. It reads `config::resolved` now.
- `config::placement::global_settings_at` (Machine names and aliases, asked
  by ownership, placement and Machine reads) resolved the global file per
  call. It is a fourth variant, `Derived::GlobalSettings { outside }`, with the store's home as `outside.home`.
- Bare `lf wave status`, `lf wave show` and `lf task list` lost the deleted
  `waves.rs` memo and resolved once per Task: 168, 121 and 172 ms against the
  base's 139, 102 and 136 ms over 30 Tasks. `snapshot_task_details` now runs
  in `observe::reading`, a reading that lasts for the call: 138, 87, 123 ms.

After the fixes, same load: 28 samples in `resolve_with`, 17 in
`validate_value` (base 4,261), server CPU 107% against 181%. `outside()` is 51
samples; `resolved_placement` is under 10, so placement stays underived.

## Built from the concept review (2026-11-18)

Nothing is left to build. The three places a computation read outside its key:

1. **A checkout's `.git` is a dependency of its settings**
   (`personal_file_layers`, `config.rs`). Not covered, on purpose: a `.git`
   file rewritten to name another repository, and the forwarded-profile path
   (`remote::layers`), which no served request takes.
2. **`GlobalSettings` has one home.** A store's home can differ from
   `LF_HOME` (it is the database's directory), so `global_settings_at` puts
   the store's home in `outside.home` and the key holds `outside` alone.
3. **`remote::apply_document` takes the profile text and the home.** The
   global computation passes `outside.remote` and `outside.home`;
   `placement::read_at` passes `incoming_text()` and its own `home`, where it
   compared against `lf_home_dir()` before.

Checks run by implement, from the checkout with `LF_*` unset:

- `cargo test -p loopflow-execution --lib -- personal_settings_reach_a_task_whose_worktree readings_share config::placement::tests config::remote::tests`:
  10 passed. `read_stream::tests::personal_settings_reach_a_task_whose_worktree_appears_after_a_reading`
  failed at `dependencies.changed()` before edit 1.
- `cargo clippy -p loopflow-execution --all-targets -- -D warnings`: clean.
  Gate runs clippy for the workspace.

## Larger than this Task

Left on `scratch/concept-review.html`, not built here:

- Validity is a fact about a file, and `resolve_layers` works it out again for
  every resolution and every skill. A `Config` carries its raw layers
  (`setting_layers`) so that a resolved value can be resolved again.
- `validate_value` deserializes a whole `Config` per leaf. Bare `lf` commands
  pay that on every invocation and nothing here changes it.
- A warning about a bad setting, found by a served read, goes to the server's
  stderr. The person editing the file in front of Desktop sees nothing.
- `outside()` runs before every lookup and stats each harness name along
  `PATH` (51 samples in qa, more than `resolve_with` now). Installed harnesses
  as an observed fact would make it a dependency and take it out of the key.
- A `Derived` variant names a value's identity, not its type; the value is
  downcast at its one call site.
- `load_wave_config` returns `Result<Option<Config>>` and never `None`.

## Docs

`rust/loopflow/src/server/README.md` (after "external observations do not
share that transaction") and the `observe` line in `AGENTS.md` describe
derived values and match the code. No page under `docs/` changes: nothing a
read returns changes.
