# Prove the complete CLI Wave-resolution matrix for reads and mutations

## Problem

W2-151 shipped one shared ambient-Wave resolver (`resolve_managed_wave_name`) and
proved it across `lf status`, `lf pm show`, and `lf memory show` — three read
consumers. But the task closed without the required full read-plus-mutation
parser matrix. Today, five commands silently bypass the shared resolver, each
inventing its own rule for Wave context. Adding or changing an `lf` command can
silently invent a different rule, and nothing in CI catches it.

## The demo

Infrastructure-only — no user-facing demo. The proof is a CI test that fails.

```bash
cargo test -p loopflow --test wave_resolution_matrix
```

Every Wave-scoped command (reads and mutations) runs across seven context
environments in one table-driven harness. All commands in the same environment
classify the same way. Adding a new `--wave`-bearing command without registering
it in the matrix fails CI.

## Approach

One Rust integration test (`tests/wave_resolution_matrix.rs`) with three layers:

1. **Command registry** — a const table enumerating every Wave-scoped command
   with its CLI args, read/mutation kind, and how to observe the resolved wave.
2. **Environment matrix** — seven context environments, each with a shared
   expected classification. The test runs every command × every environment.
3. **Completeness guard** — a test that walks the clap `Cli` tree, discovers
   every leaf with a `wave` arg, and asserts each is in the registry.

Plus: fix the five divergent commands to route through the shared resolver so
the matrix passes.

### The seven environments

| Label | `LF_WAVE_ID` | `--wave` | Expected classification |
|-------|-------------|----------|------------------------|
| registered-uuid | `<uuid in registry>` | — | `Resolved("product")` |
| registered-name | `product` | — | `Resolved("product")` |
| explicit-override | `<stale uuid>` | `product` | `Resolved("product")` |
| stale-uuid | `<uuid not in registry>` | — | `StaleIdentity` |
| stale-name | `ghost` | — | `Resolved("ghost")` then consumer fails |
| explicit-unknown | — | `ghost` | `Resolved("ghost")` then consumer fails |
| absent | — | — | `NoContext` |

"Shared classification" means every command in the same environment gets the
same classification from the resolver. `Resolved(name)` means the wave was
resolved to that name — the command may still fail downstream (no Linear token,
no registry row), but the resolution was correct. `StaleIdentity` and
`NoContext` are resolver-level errors the command surfaces.

### Classification logic

Each cell runs the `lf` binary with the right env and observes stdout/stderr/exit
code. The classifier maps the outcome to a `ResolveOutcome`:

```
fn classify(output) -> ResolveOutcome:
  if exit == 0 and wave_name in stdout: Resolved(wave_name)
  if exit != 0 and "stale" in stderr: StaleIdentity
  if exit != 0 and ("determine wave" or "no wave" or "pass --wave") in stderr: NoContext
  if exit != 0 and wave_name in stderr or stdout: Resolved(wave_name)  # post-resolution failure
  else: Unclassified (test fails)
```

The last arm is the catch: a command that silently drops a stale UUID (like
`lf home probe` today) would show "no wave given" in stderr — classifying as
`NoContext` when the environment is `stale-uuid` → mismatch → test fails.

### Command registry

Every Wave-scoped command, categorized:

**Reads (resolve and surface wave state):**

| Command | `--wave` form | Resolution path today | Diverges? |
|---------|--------------|----------------------|-----------|
| `lf status` | positional `wave` | `resolve_managed_wave_name` (async) | No |
| `lf roadmap` | `--wave` flag | Direct `get_wave_by_name` | **Yes** — no ambient, no UUID |
| `lf pm show` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf pm status` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf memory show` | `WaveTargetArgs` | `resolve_target` → resolver | No |
| `lf memory log` | `WaveTargetArgs` | `resolve_target` → resolver | No |
| `lf chat --history` | `WaveTargetArgs` | `resolve_target` → resolver | No |
| `lf receipt show` | `--wave` flag | `resolve_target` → resolver | No |
| `lf home probe` | positional `wave` | `resolve_run_wave_name` | **Yes** — drops StaleIdentity |
| `lf reviews catch-up` | global `--wave` | `ops::resolve_wave_name` (explicit-only) | **Yes** — no ambient |
| `lf thread follow` | `WaveTargetArgs` | `resolve_target` → resolver | No |

**Mutations (resolve and write to wave-scoped state):**

| Command | `--wave` form | Resolution path today | Diverges? |
|---------|--------------|----------------------|-----------|
| `lf memory add` | `WaveTargetArgs` | `resolve_target` → resolver | No |
| `lf memory update` | `WaveTargetArgs` | `resolve_target` → resolver | No |
| `lf chat <text>` | `WaveTargetArgs` | `resolve_target` → resolver | No |
| `lf chat --steer` | `WaveTargetArgs` | `resolve_target` → resolver | No |
| `lf radio pub` | `--channel`/ambient | `resolve_ambient_channel` → resolver | No |
| `lf radio sub` | positional/ambient | `resolve_ambient_channel` → resolver | No |
| `lf pm init` | `--wave`/positional | `resolve_managed_wave_name_sync` | No |
| `lf pm sync` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf pm rename` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf pm reteam` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf pm task create` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf pm task update` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf pm task done` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf pm task move` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf pm project create` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf pm project update` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf pm project archive` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf pm webhook serve` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf pm webhook register` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf cron add` | `--wave` flag | `resolve_managed_wave_name_sync` | No |
| `lf project start` | `--wave` flag | `ops::resolve_wave_name` (explicit-only) | **Yes** — no ambient |
| `lf project promote` | `--wave` flag | raw `Option::ok_or` (no resolver) | **Yes** — no ambient |

**Excluded (always-explicit, no resolution to test):**

- `lf cron sync` / `lf cron remove` — `wave: String` (required)
- `lf wave <name>` — `name: String` (required)
- `lf stop <name>` — `name: String` (required)
- `lf pm doctor` — no wave arg (global diagnostic)

### The five divergences and their fixes

1. **`lf home probe` / `lf home start`** — `resolve_run_wave_name()` calls
   `resolve_managed_wave_name_sync(None)` but catches ALL errors → `None`. A
   stale UUID looks like `NoContext` to the caller. Fix: replace
   `resolve_wave_name` in `home.rs` with a call to
   `resolve_managed_wave_name_sync(wave.as_deref())`, propagating `StaleIdentity`
   as a fatal error (same as PM's `ambient_wave` closure). `NoContext` stays
   non-fatal for the `--wave`-less case so bare `lf home probe` still works
   outside a managed process.

2. **`lf reviews catch-up`** — `ops::resolve_wave_name(explicit)` only normalizes
   the explicit `--wave` arg. Ignores `LF_WAVE_ID` entirely. Fix: replace with
   `resolve_managed_wave_name_sync(wave.as_deref())`, same pattern as PM.

3. **`lf roadmap`** — takes `--wave` but passes it directly to
   `get_wave_by_name`. No ambient resolution, no UUID support. Fix: route
   through `resolve_managed_wave_name` (async, with store). When no wave is
   resolved, keep the existing "list all waves" behavior (roadmap is the one
   command where "all waves" is a valid default, not an error — but only when
   `NoContext`, not when a stale UUID was silently dropped).

4. **`lf project start`** — `ops::resolve_wave_name(explicit)` in
   `ops/project.rs:391`. Same explicit-only bug as reviews. Fix: route through
   `resolve_managed_wave_name_sync`.

5. **`lf project promote`** — `wave.clone().ok_or(...)` in `bin/lf.rs:1372`. No
   resolver at all. Fix: route through `resolve_managed_wave_name_sync`.

### Mutation targeting proof

"Proves mutations target exactly the resolved Wave" has two tiers:

**Cache-only mutations** (`memory add`, `memory update`, `chat <text>`, `cron add`):
Seed two wave directories (A and B). Run the mutation with context resolving to
wave A. Assert wave A's journal/MEMORY.md/plist was modified and wave B's was
not. `lf cron add` writes a launchd plist with a `wave` field — point it at a
temp `~/Library/LaunchAgents` and verify the plist names wave A.

**Linear-backed mutations** (PM task/project/webhook, `project start`): In
cache-only mode, these resolve the wave, read the local PM snapshot, then fail
at the Linear API call (no token). The test verifies the resolved wave name
appears in the output or error, proving the resolution targeted the right wave
before the API failure. Seed two PM snapshots (wave A and B); a mutation
resolving to wave A reads wave A's snapshot and fails with wave A's project
context in the error, not wave B's.

### Completeness guard

A test `matrix_registry_is_complete` that:

1. Builds `Cli::command()` (clap's `CommandFactory`).
2. Recursively walks all subcommands to leaf commands.
3. For each leaf, checks if it has a `wave` arg — by `long == "wave"`, by
   positional `id == "wave"`, or by flattened `WaveTargetArgs` (which surfaces
   as `--wave` on the parent).
4. Asserts every such leaf is in the `WAVE_SCOPED_COMMANDS` registry.
5. Also checks an `AMBIENT_ONLY_COMMANDS` list (reviews catch-up, thread follow,
   radio sub) — commands that resolve ambient wave without a `wave` arg on the
   subcommand itself (they inherit the top-level `--wave`).
6. Asserts every registry entry maps to a real clap leaf (no stale entries).

Adding a new `--wave`-bearing subcommand → step 4 fails. Removing a command →
step 6 fails.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does clap's `CommandFactory` expose flattened `WaveTargetArgs`? | Yes — `#[command(flatten)]` merges args into the parent. `Cli::command().find_subcommand("memory").find_subcommand("show").get_arguments()` returns the `--wave` arg. | Completeness guard can use clap introspection directly, no help-text parsing. |
| Can PM mutations be tested cache-only? | PM commands resolve the wave, read the local SQLite snapshot, then call Linear. No Linear token → API fails. The wave name in the error proves correct resolution. | Two-tier mutation proof: side-effect verification for cache-only mutations, error-content verification for Linear-backed ones. |
| Is `lf roadmap`'s "list all waves" default valid? | Yes — roadmap is intentionally global when unscoped. But a stale UUID should error, not silently list all. | Fix: route through resolver; `NoContext` → list all (existing behavior); `StaleIdentity` → error (new). |
| Do `lf cron sync/remove` need to be in the matrix? | No — `wave: String` is required, not optional. There's no resolution to test. | Excluded from the matrix. The completeness guard recognizes required-positional as always-explicit. |
| Will 200+ CLI invocations be too slow for CI? | Each invocation is cache-only (no network), spawns a binary, runs in ~100ms. 30 commands × 7 envs = 210 invocations ≈ 21s. The existing W2-151 test runs 15 invocations in ~2s. | Acceptable. Can parallelize with `#[tokio::test]` and `tokio::process::Command` if needed. |
| Does `lf home start` actually mutate in a test env? | It SSHes to a remote host. In a test, it fails before SSH (no wave home configured). | The test verifies the wave name in the resolution error, not the SSH side effect. |
| Is `ops::resolve_wave_name` used elsewhere? | Only in `reviews.rs` and `ops/project.rs:project_start`. Both are divergences to fix. | After fixing both, `ops::resolve_wave_name` can be deleted (one implementation rule). |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Per-command snapshot tests | Blesses divergence — each command gets its own expected outcome | Task explicitly requires shared classifications, not per-command snapshots |
| Help-text parsing for completeness guard | Fragile — depends on clap's help format | Clap `CommandFactory` introspection is stable and direct |
| Mock the Linear API for mutation tests | Elaborate mock infrastructure; tests mock wiring, not behavior | Error-content verification proves resolution without mocking |
| Fix divergences first, then build matrix | Matrix is the proof that fixes are correct — building it first exposes all divergences at once | Build matrix and fixes together; matrix is the gate |
| Test only the resolver, not CLI commands | W2-151 already did this. The task requires integration proof through the CLI. | Resolver unit test stays; the matrix is the integration layer |

## Key decisions

1. **One test file, one table.** All commands × all environments in a single
   `tests/wave_resolution_matrix.rs`. The table is the contract; the
   completeness guard ensures it stays complete.

2. **Shared classifications, not per-command expected outcomes.** The expected
   outcome is per-environment, not per-command. A divergence is a test failure,
   not a blessed special case.

3. **Fix the five divergences before the matrix can pass.** The matrix is the
   proof; the fixes are the work. All five fixes route through the existing
   `resolve_managed_wave_name_sync` — no new resolver, no new abstraction.

4. **`ops::resolve_wave_name` is deleted after fixes.** It was the explicit-only
   resolver that caused two of the five divergences. After both callers route
   through the shared resolver, it has zero users.

5. **`lf roadmap` keeps "list all waves" for `NoContext`.** Roadmap is the one
   command where global scope is a valid default. But a stale UUID is an error,
   not a silent drop to global scope.

6. **Mutation targeting is two-tier.** Cache-only mutations verify side effects
   on the right wave. Linear-backed mutations verify the resolved wave name in
   the output/error. No mock infrastructure.

7. **The completeness guard walks clap, not help text.** `Cli::command()` gives
   a stable tree. The guard catches both unregistered new commands and stale
   registry entries.

## Scope

- In scope:
  - `tests/wave_resolution_matrix.rs` — the table-driven harness
  - Completeness guard test (clap tree walk)
  - Fix 5 divergent commands (home, reviews, roadmap, project start, project promote)
  - Delete `ops::resolve_wave_name` after both callers are fixed
  - Seed helper for deterministic local stores (extends the existing W2-151 `seed`)

- Out of scope:
  - Linear API mocking
  - Testing `lf wave`/`lf stop`/`lf cron sync`/`lf cron remove` (always-explicit)
  - Testing `lf pm doctor` (global, no wave scope)
  - Prompt/skill flow resolution (not CLI commands)
  - The existing `wave_resolution_tests.rs` (stays as the resolver unit test)

## Done when

```bash
cargo test -p loopflow --test wave_resolution_matrix
```

passes with:
- Every Wave-scoped read and mutation command registered in the matrix
- Each command exercised across all seven environments
- Shared classifications hold (no per-command special cases)
- Cache-only mutations prove side effects target the resolved wave
- Linear-backed mutations prove the resolved wave name in output/error
- Completeness guard discovers all `wave`-bearing clap leaves and asserts
  registry membership
- CI fails when a new `--wave`-bearing command is not registered

And:

```bash
cargo test -p loopflow --test wave_resolution_tests  # existing W2-151 tests still pass
cargo clippy -- -D warnings
cargo fmt --check
```

## Measure

Baseline: 5 divergent commands silently bypass the shared resolver. After: 0
divergences, proven by a matrix that fails on any new bypass.
