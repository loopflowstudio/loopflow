# ENG-112 — Make shared DB migrations exclusive to promoted lf

## Root defect (5-Whys, from the issue)
Binary publication and shared-store frontier advancement had no single
transactional owner. A `published`-authority candidate at `target/release/lf`
advanced `~/.lf/loopflow.db` from `.027` to `.029` by running an *ordinary*
command — never through `lf install promote` — so the installed `0.11.3`
(known-through `.027`) could no longer open the store.

Why the fence didn't catch it: migration application was an unconditional side
effect of any published open (journal setup included). The promotion fence only
gated *replacing the global binary*, not *advancing the shared store*.

## Accepted design — the shared frontier belongs to the promotion boundary alone

`FrontierAdvance { Forbidden, Authorized }` threads through the store open.
`SqliteStore::new` opens `Forbidden`; only `SqliteStore::open_as_promotion_boundary`
(called from `install::promote`'s activation, under the exclusive promotion lock
and drained live-body fence) opens `Authorized`.

Behaviour against `~/.lf/loopflow.db` (the shared release store):

- **Ordinary open never initializes or advances it.** The frontier authority is
  resolved *before* `create_dir_all`/`Connection::open`, so an absent or empty
  shared store is refused actionably without leaving a file behind — a stray
  empty `loopflow.db` must never read as "initialized".
- **Ahead ordinary candidate refuses, it does not reuse.** For an existing store,
  a `Forbidden` open validates the applied history (preserving divergent /
  incompatible / store-ahead errors), then runs one pending-frontier check: if
  this binary knows a migration the store has not applied, it refuses with an
  error naming `lf install promote`. It never hands N+1 code a store still at the
  N schema, which would only fail later when the code queries N+1 columns.
- **Exact-frontier ordinary open proceeds.** Store frontier == binary head ⇒
  nothing pending ⇒ the installed/current binary opens normally.
- **First initialization is an install operation.** `install::build_preview`
  treats an absent store, read under the exclusive promotion lock, as positive
  proof of zero persisted live leases and an uninitialized frontier: it
  classifies as `AheadPending{uninitialized}` with no live bodies, so a published
  candidate reaches `PromoteAndMigrate` and the authorized open creates the store
  during activation. An existing empty/corrupt file is *not* this case and still
  fails closed (`Incompatible`/`Unreadable` ⇒ `Reject`).
- **Validation-only builds** never write the shared store, at any advance level.
- **Private/isolated DBs** (any path that is not `~/.lf/loopflow.db`) still
  initialize and advance freely — the isolated dev-DB escape.

## Surface
- `store::FrontierAdvance` + `may_apply_migrations(path, authority, home, advance)`.
- `SqliteStore::open` resolves authority before any filesystem mutation; the
  ordinary-shared path validates then runs `migrations::pending_shared_migration`.
- `install::read_store_evidence` (authority-free) feeds `build_preview`; an absent
  store maps to a promotable uninitialized frontier.

## Proof
- `may_apply_migrations`: shared advancement authorized only at the boundary;
  private always free; validation-only always walled.
- `install::tests`: absent store ⇒ promotable uninitialized frontier (published
  `PromoteAndMigrate`, validation-only `Reject`); existing empty file fails closed.
- `store::sqlite::frontier_tests` (real `SqliteStore` open, injected temp home +
  authority): (a) absent ordinary open creates nothing and refuses; (b) existing
  N + ordinary N+1 open refuses naming `lf install promote`, leaves frontier N,
  and the old N reader still recognizes it; (c) the boundary initializes an absent
  store and advances an N store to the head, after which an ordinary open proceeds.
- `store::migrations::tests`: the `validate_sqlite` primitive recognizes a shorter
  frontier and pins `pending_shared_migration` to the exact head the ordinary open
  refuses on, without advancing.
