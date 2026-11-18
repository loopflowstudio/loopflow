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

## Design: derived values in `observe`

`observe` already is the one notion of "the files changed": a served reading
runs inside `Cache::capture`, each `observe::read_to_string` / `read_dir` /
`is_file` returns the cached answer and records `(entry, revision)` in the
reading's `Dependencies`; the background refresh bumps a revision when an
answer differs (`rust/loopflow-execution/src/observe.rs`). A subscription reads
again when `Dependencies::changed()`.

Add the next layer on the same mechanism: a value computed from observed facts
is kept with the dependencies its computation read.

```rust
/// `compute`'s result, shared until a fact it observed changes.
/// `compute` may read only observed facts and `key`: never the store,
/// the environment or thread-local state.
pub fn derived<K, T>(key: K, compute: impl FnOnce() -> T) -> Arc<T>
```

- **No reading active** (bare CLI): `Arc::new(compute())`. No cache exists.
- **Reading active, hit**: the slot's dependencies are unchanged, so return the
  shared value and add those dependencies to the current reading. The
  subscription still wakes when one of the files changes.
- **Miss or changed**: run `compute` under a child reading (same `Cache`, fresh
  `Dependencies`), store value and dependencies, add them to the parent. A
  derived value nested in another therefore feeds the outer one's dependencies.
- The slot is locked while computing, so four detail workers asking for one
  repository's settings compute once.
- Slots live in `Cache` and leave by the rule fact entries already use (unused
  for 300 s). A slot's dependencies hold their entries (`strong_count > 1`), so
  the existing refresh keeps checking those files. No watcher, no timer, no
  size knob.
- Staleness is what served reads already have: the Local refresh interval (1 s).

Why this and not a content-keyed memo of `resolve_layers`: that one is immune
to unobserved inputs, but still parses every file per read, needs its own
eviction policy, and does nothing for the catalog's directory walk. Passed over.

Why not move filtering to where each file is read (one derived per file, which
is observed by construction): layers are plain values that callers slice
(`private_wave_layers`), prune (`ContextBudgets::load`) and rebuild
(`store/placement.rs`); moving validation changes which declarations are
checked under which scope, the semantics `wave_setup_tests::` pins. Passed over.

### The rule that makes it safe, and the audit behind it

A derived closure that reads something unobserved goes stale until one of its
files changes. Today such inputs are picked up by the next store revision; with
this change they would not be. Findings, each with its answer:

| Input | Where | Answer |
|---|---|---|
| `--setting` overrides (`INVOCATION` thread-local) | `settings.rs:979` | Not derived when non-null: compute inline. Never enters a shared slot. |
| `LF_SETTINGS`, `LF_AGENT` etc. | `settings::environment()` | Empty inside a served request already; its value goes in the key for `--subscribe` processes. |
| Sole installed harness (PATH probe) | `config::default_agent()` | In the key. Probed per call as today. |
| `LF_HOME` | `global_config_path()` | In the key (fixtures share one `Cache` across homes). |
| Remote profile env | `remote::incoming()` | `None` inside a served request; value in the key otherwise. |
| Wave named by ID walks `.lf/wave` with `ignore` | `files::wave_name` | Resolve the name before `derived`; key on the name. |
| Main checkout of a worktree (`std::fs` in `git::worktree_root`) | `files::private_path` | Fixed for a checkout path; deleting the checkout changes `.lf/config.yaml`, which is a dependency. |
| `fs::canonicalize` on skill dirs | `catalog.rs:330,354` | Symlink retargets show as changed `Metadata`/`File`/`Directory` answers for the lexical path. |
| `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `provider_overrides` | `SkillCatalog::collect_scope` | In the catalog key. |
| Store rows (placement scope chain, Wave rows) | `store/placement.rs`, `wave/setup.rs::machine` | Read outside; the chain is the key. |

Everything else on these paths already goes through `observe`
(`load_yaml_file`, `files::read_text`, `read_wave_file`, catalog `collect`,
`is_generated`, `SkillSource::read`).

Launches and mutations are not affected: only `Cache::capture` readings
(`read_stream.rs:610`, `server/service.rs:1432`, `session.rs:436`) have a cache,
so a stale value could reach a display, never a launched agent.

### What is derived

1. **Settings for a checkout and Wave**: `config::resolved(repo, wave) ->
   Result<Arc<ResolvedSettings>, LoadError>` is `config_layers` +
   `resolve_layers(_, None)`. `load_wave_config`, `load_config`,
   `scope_graph_default`, `read_wave_setup` and `Config`-returning helpers read
   from it; layers come from `config.setting_layers`. Errors are part of the
   value, so an invalid file reports the same error on every read and is
   resolved again when it changes. `LoadError` is not `Clone`; the slot keeps
   the message, as `PersonalSettings` does now.
2. **A Task checkout's agent** (`resolve_task_agent` with no skill): today two
   full resolves per Task per reading (`load_wave_config_with`, then
   `settings::resolve` over `setting_layers`). It reads the agent from (1).
3. **A Wave's per-skill agents** (`wave/setup.rs:48-57`, one `resolve_layers`
   and one catalog load per skill named in any layer): one derived value for
   the settings half of `WaveSetup`; `machine()` stays outside.
4. **Machine placement** (`store/placement.rs:81`): derived by `(home, scope
   chain)`.
5. **The skill catalog**: `SkillCatalog::load(repo, home, overrides)` returns
   the shared catalog (`Arc<SkillCatalog>`); the walk and each `is_generated`
   front matter parse run once per change under `.lf/skills`, `.claude`,
   `.agents`, `.codex`. `load_skill` on top stays per call unless slice 1's
   sample still shows it.

File warnings (`reader::warn`, stderr) are printed when a value is computed, so
the server says each once per file change instead of once per read.

## What goes

Two earlier per-reading caches for this same cost, superseded by (1) and (2):

- `config::PersonalSettings`, `load_wave_config_with`, `config_layers_with`
  and the `personal` parameter threaded through `resolve_task_agent`,
  `task_configuration_refusal`, `task_process_refusal_after`
  (`ops/task.rs:2274-2318`), `ops/task/snapshot.rs:32`, `session.rs:15`.
- In `rust/loopflow/src/lf/commands/waves.rs`: the `personal` map
  (`:1873-1884`, filled at `:1953-1963`) and the `agents` memo keyed by
  serialized layers (`:1998-2027`).

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
untouched. Break `derived` to always compute and the count fails; break
dependency propagation and the existing "changes reach" tests hang.

`cargo test -p loopflow --test integration wave_setup_tests::` passes.

## Demo

On a Machine under the same kind of load, with this branch's `lf` as the
server: `sample $(pgrep -f 'lf server run') 3 | grep -c validate_value` before
and after, and the CPU column of `ps`. Before: ~1,700 samples. After: none
outside the second after a config edit. The numbers go in the PR body.
Installing a branch build over a server with live Sessions is a person's call;
the headless stand-in is slice 1's timing.

## Slices

1. **`observe::derived` and settings (1).** The primitive with unit tests in
   `observe.rs` (hit, invalidation by a changed fact, nested dependencies
   reaching the outer reading, bare call computes inline, concurrent callers
   compute once), then `config::resolved` under `load_wave_config`. Timing in
   the acceptance test's shape: 1,000 forced readings of `load_wave_config`
   over three layers, before and after. This is the slice that could prove the
   plan wrong: if a "changes reach" case fails, an input is unobserved and the
   table above is missing a row.
2. **Task agents and Wave setup (2, 3)**, deleting `PersonalSettings`, the
   `_with` variants and both `waves.rs` maps in the same change.
3. **Skill catalog (5).**
4. **Placement (4)**, then a `sample` of a locally run server reading a seeded
   store to see what of `validate_value` and `collect` is left, and the
   `observe` line in `AGENTS.md` gains derived values.

Checks: slice tests run locally by the implementing step; clippy and the full
matrix at gate/CI; the live `sample` by a person.
