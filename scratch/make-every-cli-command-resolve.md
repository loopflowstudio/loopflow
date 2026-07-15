# W2-151 — Make every CLI command resolve managed Wave context consistently

## The reframe

The stale diagnosis blamed the resident launcher for "exporting a UUID as
`LF_WAVE_ID`." That is not a bug — a durable registry UUID is exactly the right
thing to inherit. The bug is that **consumers disagree about how to read it**.

Evidence, one env (`LF_WAVE_ID=<uuid>`, no `--wave`), three behaviors:

| Consumer | Resolves `LF_WAVE_ID` | Behavior on a UUID | Behavior on a hand-set name |
|---|---|---|---|
| `lf status` (`resolve_status_wave`, `waves.rs:691`) | UUID-first `get_wave`, then `get_wave_by_name` fallback, stale-context error | ✓ resolves | ✓ resolves |
| `lf pm show` (`ops/pm.rs::resolve_wave:473`) | **ignores env** — `resolve_wave_name(--wave)` only | ✗ errors "cannot determine wave; pass --wave" | ✗ same |
| `lf chat` / `lf radio` / `lf memory` (`resolve_ambient_channel` → `get_wave(id.parse()?)`) | UUID-only via `resolve_ambient_channel`'s `WaveId` arm | ✓ resolves | ✗ `id.parse()` fails → silent drop / read error |
| `lf home`, trace attribution (`resolve_run_wave_name` → `wave_name_for_id:196`) | UUID-only (`id.parse().ok()?`) | ✓ resolves | ✗ falls back to worktree name |

`status` already encodes the target contract. Everything else is a partial or
absent copy. The fix is one resolver, applied everywhere — not five independent
patches.

## User-visible outcome

Inside a Wave, Project, or Task process, **every** `lf` command that acts on the
ambient Wave resolves the same durable Wave — no `--wave`, no UUID-as-name
confusion, no per-subsystem failure. Whoever runs `lf pm show`, `lf status`,
`lf chat`, `lf radio pub`, `lf memory show` from resident Product (or its Mac
Project Session) sees the same Wave. Explicit `--wave` still wins everywhere and
stays predictable for scripts outside managed context.

## Source of truth

The **shared SQLite registry row** (`Wave`, id + name) is Wave identity. The
environment (`LF_WAVE_ID`) is only a *pointer used to find that row* — never
identity itself. All views (PM name lookup, status, chat channel, memory file,
trace attribution) derive from the resolved row.

## The one resolver

Add to `engine/wave_context.rs`:

```rust
pub enum WaveResolveError {
    NoContext,                 // no --wave and no LF_WAVE_ID
    StaleIdentity(String),     // LF_WAVE_ID present but not in this registry
    UnknownExplicit(String),   // --wave <name> not found
}

// Async core: for consumers that already hold a SharedStore (chat/radio/memory).
// explicit --wave always wins; else LF_WAVE_ID: UUID-first, name fallback.
pub async fn resolve_managed_wave(
    store: &SharedStore,
    explicit: Option<&str>,
) -> Result<Wave, WaveResolveError>;

// Sync wrapper: opens the store on a scratch thread (the existing
// `resolve_explicit_wave` / `wave_name_for_id` idiom) for sync, store-free
// call sites like `run_pm`. No store on this machine ⇒ NoContext.
pub fn resolve_managed_wave_sync(
    explicit: Option<&str>,
) -> Result<Wave, WaveResolveError>;
```

Resolution order (the contract, verbatim):
1. **explicit `--wave`** → `normalize_wave_name` → `get_wave_by_name`;
   `None` ⇒ `UnknownExplicit`. Always wins.
2. **`LF_WAVE_ID` as durable UUID** → if it parses as `WaveId`, `get_wave`;
   `Some` ⇒ row. A UUID-shaped id that misses the registry ⇒ `StaleIdentity`.
3. **`LF_WAVE_ID` as hand-set name** (intentional fallback) → not UUID-shaped ⇒
   `get_wave_by_name`; `Some` ⇒ row, `None` ⇒ `StaleIdentity`.
4. nothing ⇒ `NoContext`.

This is `resolve_status_wave` generalized: + name-fallback for the UUID-miss
path is deliberately *stale*, not *retry-as-name* (a real UUID that misses means
the context outlived its registry, exactly the stale case).

`resolve_status_wave` is deleted and re-expressed as a thin call to this.

## Where the resolver runs (keeps ops registry-free)

PM ops must **not** grow a store dependency — they read `GOAL.md` by wave *name*
and must stay daemon-less / cache-only. `run_pm` (`lf/commands/ops/mod.rs:373`)
is **sync and holds no `CliContext`/store** — only `repo_root`. So the seam
calls `resolve_managed_wave_sync(explicit)`, which opens the local store on a
scratch thread (exactly as `resolve_explicit_wave` / `wave_name_for_id` already
do), resolves the row, and passes its resolved **name** down into
`pm_show`/`pm_update`/… unchanged. `ops/pm.rs::resolve_wave` keeps taking a name;
the seam guarantees a name is always supplied inside managed context.

Async consumers that already hold a `SharedStore` (`chat`, `radio`, `memory`
via `resolve_target`/`ambient_wave`) call the async core directly — no scratch
thread. Their `LF_CHANNEL` arm is untouched; only the `WaveId` arm routes
through the resolver so a hand-set name resolves and errors classify.
`resolve_status_wave` (`lf status`) already holds a store and becomes a thin
call to the async core.

## Cache-only preservation

`lf pm show --no-sync --json` inside managed context: the name resolution is a
**local SQLite `get_wave` / `get_wave_by_name`** — not Linear, not a guessed cwd.
`--no-sync` still never touches the network. Managed processes always have the
local registry, so the resolver never needs Linear to turn a UUID into a name.

## Absent / error states per boundary

- **PM read** (`pm show`): `NoContext` ⇒ existing "pass --wave" message;
  `StaleIdentity` ⇒ "ambient wave `<id>` is not in this machine's registry; the
  context is stale — pass a wave name" (mirror status's wording);
  `UnknownExplicit` ⇒ "wave `<name>` is not linked / not in the registry".
- **PM mutations** (`project create/update`, `task …`): same classified errors,
  raised **before** any Linear write.
- **chat / radio pub**: publish keeps its drop-with-exit-0 semantics on
  `NoContext` (no subscriber); `StaleIdentity` is a loud error (context is wrong,
  not absent). Reads (`chat --follow`, `memory show`) treat `NoContext` as error.
- **trace attribution / home**: `NoContext`/`StaleIdentity` ⇒ omit wave
  attribution (unchanged non-fatal behavior), but a hand-set name now resolves.

## Affected surfaces & consumers

- `ops/pm.rs` reads + every mutation (via the CLI seam).
- `lf status` (`resolve_status_wave`).
- `lf chat`, `lf radio pub/sub` (`resolve_target`, `ambient_wave`).
- `lf memory show/log/add`.
- `lf home`.
- trace attribution (`journal/mod.rs:525`, `bin/lf.rs:387`,
  `resolve_run_wave_name`).
- No wire DTO changes: resolution is upstream of every `--json` shape. The
  `PmShowResult` envelope is untouched. Swift/iOS consume the same JSON.

## End-to-end proof

A **command matrix** test (`tests/wave_resolution_tests.rs`): the relevant read
and mutation resolvers run from 7 environments —

`Wave` · `Project` · `Task` (each `LF_WAVE_ID=<uuid>`) · `hand-set-name`
(`LF_WAVE_ID=<name>`) · `explicit-override` (`--wave` beats a wrong env) ·
`stale-ID` (`LF_WAVE_ID=<uuid not in registry>`) · `no-context` (neither set).

For each cell: every command resolves the **same Wave** or returns the **same
classified error**. Concretely asserts `pm show` and `status` agree in all seven.
The existing `ambient_wave_id_resolves_the_wave_it_names` (`status_tests.rs:291`)
extends to cover `pm show` from the same resident env — the original repro, now
green from both resident Product and the Mac Project Session.

Command to prove PR1:
`cargo test -p loopflow --test wave_resolution_tests` and
`cargo test -p loopflow --test status_tests ambient_wave_id_resolves_the_wave_it_names`.

## Operational boundary

Resolution is one local SQLite lookup (`get_wave` / `get_wave_by_name`) —
daemon-less, no network, sub-millisecond. It must never introduce a Linear call
on the read path, and must not gate on the bundled daemon.

## Serial PR plan (one worktree, ordered branches)

- **PR 1 — the repro + the spine.** `resolve_managed_wave` + `WaveResolveError`
  in `wave_context`; route `lf status` and PM reads+mutations through it at the
  CLI seam; the command-matrix test. Closes the `lf pm show` UUID repro.
  `--next chat-radio-memory`.
- **PR 2 — channel + memory consumers.** Route `chat`, `radio`, `memory`'s
  `WaveId` arm through the resolver (hand-set-name fallback + classified errors);
  extend the matrix to them.
- **PR 3 — trace + home.** `resolve_run_wave_name` / `wave_name_for_id` gain the
  name fallback via the shared resolver; matrix covers trace attribution.

## Exclusions

- No change to `LF_CHANNEL` semantics or sub-channel (`family_head`) derivation —
  only the `WaveId` arm is unified.
- No new wire DTO fields; no Swift/iOS model change (JSON shape unchanged).
- Not touching how the launcher *sets* `LF_WAVE_ID` — exporting the UUID is
  correct; consumers were the bug.
- Machine-wide roadmap aggregation (W2-144) and liveness (W2-139) are separate.
