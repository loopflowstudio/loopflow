# Reject unknown explicit Wave names consistently across all consumers

W2-240. Discovered after Product W2-151 merged the shared ambient-wave
resolver: the resolver unifies the *ambient* matrix but leaves the *explicit*
`--wave` arm unvalidated, so an unknown explicit name is accepted by some
consumers, rejected with a generic message by others, and misdirected to a
sync command by a third.

## Problem

`engine/wave_context.rs` names `WaveResolveError::UnknownExplicit` but the
variant only ever fires on **empty** input. A nonempty explicit `--wave
definitely-unknown` is returned as `Ok(name)` with no registry check. Each
consumer then handles the phantom name its own way.

Live reproduction (confirmed on merged main, `LF_WAVE_ID=product`):

| Command | Behavior | Source path |
|---|---|---|
| `lf memory show --wave definitely-unknown` | exit 0, empty output (silent accept) | `resolve_target` → `row=None`, name used directly |
| `lf status --wave definitely-unknown` | `Error: wave 'definitely-unknown' not found` | global `resolve_explicit_wave` (registry-validating, generic) |
| `lf pm show --wave definitely-unknown` | `no local PM snapshot… Run \`lf pm sync --wave definitely-unknown\`` | shared resolver accepts name → PM lookup misdirects to syncing a nonexistent wave |

An operator who typos a wave name gets three different stories. The worst is
memory's: it succeeds with empty output, so the failure is invisible. The
second-worst is PM's: it tells you to `lf pm sync` a wave that does not exist,
sending you down a dead end.

Who benefits: anyone steering waves from the CLI — operators, agents in
headless runs, and the conductor surface that shells out to `lf`.

## The demo

```
$ lf memory show --wave definitely-unknown
Error: wave 'definitely-unknown' is not registered on this machine.
Run `lf ls` to list known waves, or pass --wave <known-name>.

$ lf status --wave definitely-unknown
Error: wave 'definitely-unknown' is not registered on this machine.
Run `lf ls` to list known waves, or pass --wave <known-name>.

$ lf pm show --wave definitely-unknown
Error: wave 'definitely-unknown' is not registered on this machine.
Run `lf ls` to list known waves, or pass --wave <known-name>.
```

Same words, same exit code, same classification — from every consumer. A valid
name still works exactly as before:

```
$ lf memory show --wave product
PRODUCT MEMORY
…
```

## Approach

One classified error, produced by one shared rule, surfaced by every consumer.

### 1. The shared resolver validates the explicit arm

`resolve_managed_wave_name` / `resolve_managed_wave_name_sync` gain registry
validation on the **explicit** arm only. The ambient arms are unchanged
(W2-151's intentional fallbacks stay):

- **Explicit nonempty** → normalize → look up in the registry →
  `Ok(name)` if found, `WaveResolveError::UnknownExplicit(name)` if not.
- **Explicit empty/whitespace** → new `WaveResolveError::EmptyExplicit`
  (distinct invalid-input error — not `UnknownExplicit`).
- **Ambient UUID** → map through store (existing; `StaleIdentity` on miss).
- **Ambient hand-set name** → resolve to itself, no membership (existing;
  the W2-151 test at `wave_resolution_tests.rs:86-94` keeps passing).
- **No context** → `NoContext` (existing).

The explicit arm now needs a store. The async signature already receives
`store: Option<&Store>`; the `_sync` variant opens one on its scratch thread
(the idiom already used for the UUID arm). When no store exists on the
machine and an explicit name is given, return a registry error — consistent
with the global `resolve_explicit_wave`, which already errors "no wave
registry on this machine" in that case. A machine with no registry has no
valid wave names.

### 2. Repurpose `UnknownExplicit`; add `EmptyExplicit`

`WaveResolveError` today:

```rust
NoContext, StaleIdentity(String), UnknownExplicit(String), Registry(String)
```

`UnknownExplicit`'s message is "--wave requires a non-empty wave name" — it
describes the empty case, not the unknown case. Split them:

```rust
/// `--wave` was given but empty/whitespace after normalization.
#[error("--wave requires a non-empty wave name")]
EmptyExplicit,

/// `--wave` named a wave this machine's registry has no row for.
#[error("wave '{0}' is not registered on this machine; run `lf ls` to list known waves, or pass --wave <known-name>")]
UnknownExplicit(String),
```

`UnknownExplicit` becomes the unknown-nonempty classification the directive
names. `EmptyExplicit` is the distinct invalid-input error. The existing
`wave_resolution_tests.rs:80-84` assertion (`Some("  ")` → `UnknownExplicit`)
moves to `EmptyExplicit`.

### 3. Migrate the divergent consumers to the shared rule

Four resolution paths exist today. Three must converge on the shared
resolver's `UnknownExplicit`:

| Path | Consumers today | Fix |
|---|---|---|
| Global `resolve_explicit_wave` (`bin/lf.rs:1261`) | any `lf --wave X <cmd>`; `lf status --wave X` (hoisted by `reorder_args`) | Surface `UnknownExplicit` classification on the not-found arm (same message + safe next action) instead of the generic `wave '{name}' not found`. Keep returning the `Wave` row on success. |
| Shared `resolve_managed_wave_name[_sync]` | `pm`, `cron`, `status` (positional) | Gets validation for free once the resolver validates the explicit arm. |
| `resolve_target` (`chat.rs:386`) | `chat`, `memory`, `thread`, `receipt` | The explicit `args.wave` arm takes `family_head(name)` (channels are `wave.runid`), so it validates *inline*: look up the head in the registry; `None` → `UnknownExplicit(head)`; no store → registry error. Not routed through `resolve_managed_wave_name` (which validates the raw name, not the family head). Today an unknown name yields `row=None` and proceeds silently; it must error with `UnknownExplicit`. |
| `home.rs:resolve_wave_name` (line 34) | `lf home probe/start` | Replace the bespoke `wave.or_else(resolve_run_wave_name)` with the shared resolver (explicit → validate; ambient → existing rule). Home never used the shared resolver — migrate it in. |

### 4. Creation flows bypass explicit validation

Not every `--wave`/name is a read. A handful of commands *create* or *start*
a wave by name and must accept a name the registry has never seen:

- `lf wave <name>` — positional, runs `wave::run`; registers the wave row on
  first run.
- `lf stop <name>`, `lf resident <name>` — positional lifecycle.
- `lf home start <name>` — positional, starts a wave's Home by launching
  `lf wave <name>` (which registers the row). The `--wave` *flag* form does
  **not** reach here: it hoists to the global `resolve_explicit_wave`
  (confirmed below), which already rejects unregistered names today — so the
  flag form was never a creation path. Only the positional form bypasses.
  (Probe is a read in both forms: flag → global validates; positional →
  `home.rs` migrates to the shared resolver with validation.)
- `lf pm init --wave <name>` / positional `<name>` — links a wave to its
  Linear Initiative. `pm_init` requires the wave *directory*
  (`wave/<name>/`), not a registry row, and resolves the name with
  normalize-only (`ops::util::resolve_wave_name`). Its `--wave` flag stays
  pm-local (confirmed below), so the global validating path never sees it.

These keep using the name directly (or a non-validating normalize) — and
**none of them route through the shared resolver's explicit arm**, so the
resolver needs no opt-out parameter. Verified: `lf wave`/`stop`/`resident` are
positional-direct (`wave::run`/`stop`/`resident::run` take the name raw);
`lf pm init` resolves with normalize-only (`ops::util::resolve_wave_name`);
the only creation flow near the resolver is `lf home start <name>`
(positional), which keeps a bespoke normalize+ambient path
(`home.rs:resolve_wave_name`) rather than calling the validating resolver.
`lf home probe <name>` (positional) *does* call the shared resolver and
validates; its `--wave` flag form is already gated by the global path.

Dropping `allow_unregistered_explicit` avoids threading `false` through ~8
read-only call sites for a single `true` that never arrives. The exception
(start bypasses) stays visible at the call site — `start_cmd` calls a
different resolver than `probe_cmd`.

Ambient hand-set names are already unvalidated and stay that way — the
directive scopes validation to *explicit* names only.

### 5. The consumer matrix

Every consumer in the directive's list, and the path that fixes it:

| Consumer | Resolution path | Fix |
|---|---|---|
| Memory | `resolve_target` | explicit arm → shared resolver validation |
| Status | global (`--wave`) + `resolve_status_wave` (positional) | global surfaces `UnknownExplicit`; positional gets it from the shared resolver |
| PM | `resolve_managed_wave_name_sync` | gets validation for free |
| Chat | `resolve_target` | same as memory |
| Radio | ambient only (no explicit `--wave` arm) + global path | covered by global unification |
| Trace | ambient attribution (`resolve_run_wave_name`) + global path | covered by global unification |
| Home | `--wave` flag → global `resolve_explicit_wave` (already validates today; surfaces `UnknownExplicit`); positional → `home.rs:resolve_wave_name` | global path: message-only change to `UnknownExplicit`. `home.rs`: migrate to shared resolver — `probe_cmd` validates (`allow_unregistered_explicit: false`), `start_cmd` bypasses (`true`, the creation path that launches `lf wave <name>`). |
| Cron | `resolve_managed_wave_name_sync` | gets validation for free |
| Task | no `--wave` flag; wave from session/ambient + global path | covered by global unification |
| Project | no `--wave` flag (except `promote --wave`); global path | covered by global unification |

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Does validating the explicit arm break wave creation? | Yes — `lf wave <name>`, `lf home start <name>` (positional), `lf pm init` accept names not yet registered. The `--wave` *flag* form of `lf home start` already validates via global today (not a creation path). | Add `allow_unregistered_explicit` opt-out; creation flows pass `true`. Validation is the default. |
| Can every read consumer open a store for explicit validation? | The async resolver receives `store: Option<&Store>`; `_sync` opens one on a scratch thread (existing idiom). `resolve_target` already has `store: Option<&SharedStore>` in scope. | No new plumbing; the explicit arm reuses the store that's already there or opens one. |
| What if there's no registry on the machine and `--wave` is passed? | `resolve_explicit_wave` already errors "no wave registry on this machine". | Be consistent: explicit + no store → `Registry` error, never a silent accept. A machine with no registry has no valid wave names. |
| Does `--wave` hoist to global for every command? | **Verified against source.** `reorder_args` is derived from the clap definition itself (`arg_tables()` at `bin/lf.rs:102` builds `CommandArgTables` from `Cli::command()`), so the local-vs-global split is automatic and can't drift from the CLI. A subcommand that declares its own `--wave` (`pm show`, `pm init` via `wave_flag`) keeps it local; one with only a *positional* `wave` (`status`, `home probe/start`) lets `--wave` hoist to global `cli.wave` → `resolve_explicit_wave`. | No hand-maintained lists to update. The matrix below records the verified path per consumer. `lf home --wave X` already validates today via global (rejects unregistered) — the design only changes its message to `UnknownExplicit`, no behavior shift. `lf pm init --wave new-wave` stays pm-local and never touches the global gate, so creation is safe. |
| Does the ambient hand-set name (`LF_WAVE_ID=ghost`) need validation? | No — W2-151 deliberately leaves it as an intentional fallback (test at `wave_resolution_tests.rs:86-94`). The directive scopes validation to *explicit* names. | Ambient arms unchanged; only the explicit arm gains validation. |
| Is `resolve_explicit_wave`'s `anyhow` error string load-bearing anywhere? | It's a user-facing stderr string. No code matches on it. | Safe to replace with the `UnknownExplicit` Display. |
| Does `resolve_target`'s `row=None` silent-accept path serve a real purpose? | It lets a publish land on a wave with no registry row (e.g., a fresh machine). But an *explicit* unknown name is never that case — it's a typo. | Only the *explicit* arm of `resolve_target` errors; the ambient `row=None` path (no wave context) stays `Ok(None)` (the "no subscriber" drop). |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Validate in each consumer, not the resolver | 10 duplicate membership checks, 10 chances to drift. | The directive names the shared resolver as the owner. Per-consumer checks recreate the W2-151 bug it fixed. |
| Two resolvers: `resolve_explicit_wave` (validating) for reads, `resolve_managed_wave_name` (permissive) for creation | Splits the rule in two; the global path and the shared path already diverge — this widens the split. | One rule with an explicit opt-out is simpler and keeps the matrix in one place. |
| Make `--wave` a global clap arg everywhere | Would unify the parse, but `reorder_args` already hoists per-command; changing clap structure risks every command's arg surface. | Out of scope; the fix is in resolution, not parsing. |
| Reject unknown ambient hand-set names too | Breaks the intentional `LF_WAVE_ID=<name>` fallback and the W2-151 contract. | The directive scopes validation to *explicit* names. |

## Key decisions

- **`UnknownExplicit` = nonempty-unregistered; `EmptyExplicit` = empty.** The
  variant name finally matches its meaning. Empty stays a distinct
  invalid-input error per the directive.
- **Validation is the shared resolver's default for the explicit arm — no
  opt-out parameter.** No creation flow routes through the resolver's explicit
  arm (verified post-rebase), so `allow_unregistered_explicit` would thread
  `false` through ~8 read sites for a `true` that never arrives. `lf home
  start` keeps a bespoke normalize+ambient path; `probe` calls the validating
  resolver. The exception is visible at the call site (different resolver),
  not hidden in a second function or a dead parameter.
- **The global `resolve_explicit_wave` surfaces the same `UnknownExplicit`
  classification.** One message, one exit code, from every path. It keeps
  returning the `Wave` row on success (it needs the row to set `LF_WAVE_ID`).
- **No-store + explicit → error, not silent accept.** Consistent with the
  global path. A machine with no registry has no valid wave names.
- **Ambient arms are untouched.** Only the explicit arm changes. W2-151's
  matrix and its hand-set-name fallback keep passing unchanged.

## Scope

- In scope:
  - `WaveResolveError`: add `EmptyExplicit`, repurpose `UnknownExplicit`.
  - `resolve_managed_wave_name` / `_sync`: validate the explicit arm against
    the registry (no opt-out parameter — see Key decisions).
  - `resolve_explicit_wave` (global): surface `UnknownExplicit` on not-found.
  - `resolve_target` (chat/memory/thread/receipt): validate the explicit
    `family_head(name)` inline; unknown → `UnknownExplicit` (no more silent
    `row=None`).
  - `home.rs`: `probe_cmd` → shared resolver (validates positional + ambient);
    `start_cmd` → keep bespoke normalize+ambient bypass (creation).
  - Creation flows (`lf wave`, `lf stop`, `lf resident`, `lf pm init`): no
    change — they never route through the shared resolver's explicit arm.
  - Subprocess integration test: the W2-240 reproduction + full consumer
    matrix in `wave_resolution_tests.rs`.
- Out of scope:
  - Changing the clap `--wave` argument structure or `reorder_args`.
  - Validating ambient hand-set names (`LF_WAVE_ID=<name>`).
  - The `lf ls` / registry contents themselves.

## Done when

- The shared resolver (`resolve_managed_wave_name` / `_sync`) returns
  `UnknownExplicit(name)` for every nonempty explicit name not in the local
  registry, and `EmptyExplicit` for empty/whitespace explicit input.
- Memory, status, PM, chat, radio, trace, home, cron, Task, and Project
  consumers return the same `UnknownExplicit` Display and exit non-zero for an
  unknown explicit name — verified by a subprocess integration test that runs
  each consumer with `--wave definitely-unknown` and asserts identical stderr
  classification.
- Explicit valid names still override stale ambient context (the W2-151
  override test keeps passing).
- Empty explicit input is a distinct `EmptyExplicit` error, not
  `UnknownExplicit`.
- A subprocess integration test preserves the live W2-151 reproduction (the
  existing `pm_show_and_status_agree_from_a_resident_uuid` and
  `memory_show_resolves_like_status_across_environments`) **and** adds the
  W2-240 consumer matrix: every consumer + `--wave definitely-unknown` → same
  `UnknownExplicit` stderr + non-zero exit; every consumer + `--wave
  definitely-unknown` with a valid ambient → explicit still wins with the same
  error (unknown is unknown, ambient doesn't rescue it).
- `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` pass.
