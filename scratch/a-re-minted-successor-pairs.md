# A re-minted successor pairs a fresh base with a stale branch

## Problem

A Task whose work has merged cannot complete, reopens itself, and re-mints its
successor forever. W2-300 is in this loop now: its real work merged as #1050
(`0fef6d2ce`), yet `status=waiting` with

```
reopened: completion outran its gates (cannot prove unpublished pull request
sequence 2 is empty: the recorded base is not an ancestor of the unpublished branch)
```

The mint records `base_commit = origin/main`'s tip while reusing a branch ref it
never moves. The completion gate then asks "is base an ancestor of branch?" about
a base that is a **descendant** of the branch. `is_ancestor` is false, the cut
reads `Unprovable`, completion fails closed, the Task reopens, and the successor
is re-minted against an even newer main. Every merge to main widens the gap, so
it never converges.

This is the ENG-7 burn by a new mechanism: a Task that can never complete and
mints a body per cycle, unbounded. It is worse now, not better — ENG-4's shipped
lease release (#1055, installed 08:11:57Z) frees the revoked lease on the next
launch attempt, which **enables** the burn a pinned lease was accidentally
suppressing. That directly attacks Developer Efficiency's KR: *"No Task strands
on a dead body: zero Sessions sit in failed awaiting a manual resume."*

### Premise verified before designing

Every claim in the directive was checked against the live store and git, not
taken as given:

| Claim | Verified |
|---|---|
| sequence 2 `base_commit` | `3e9df06777297cd1cb83fccc1d0261fd3e74dfa8` ✓ |
| branch `jack-heart/do-not-complete-a-task-2` tip | `9f6bdd4986c29a111726758701d7729781680e4a` ✓ (local **and** remote) |
| `git merge-base --is-ancestor <base> <branch>` | **false** ✓ |
| the reverse (`branch` ancestor of `base`) | **true** — the base is a *descendant* of the branch |
| worktree `/Users/jack/src/loopflow.do-not-complete-a-task` | checked out on `…-2` at `9f6bdd498` ✓ |

The gate's message reproduces byte-for-byte from `ops/task.rs:4208` +
`ops/task.rs:3464`, closing the diagnosis.

## The demo

On the fixed binary, W2-300's completion cut reads `ProvenEmpty` instead of
`Unprovable`: `lf task complete W2-300` succeeds, the successor is discarded in
the terminal write, and the Task **stays** completed — no reopen, no re-mint, no
replacement row. Measured against the live incident, not a fixture.

## Approach

### The mint violates a contract the codebase already states twice

`unpublished_work` (`ops/task.rs:3455-3458`) *defines* what `base_commit` means:

> The cut is the fork point recorded when the PR was minted, so commits past it
> are this PR's own work and `ProvenEmpty` means the branch never moved off its base.

And `verify_task_pr_range_with_authority` — the "core parity proof" run before
every publish — **already asserts that invariant** (`ops/task.rs:1492`):

```rust
let merge_base = crate::engine::git::merge_base(repo, &upstream, &head)?;
if merge_base == base {
    // Parity holds: the GitHub range is exactly base_commit..HEAD.
    return Ok(());
}
```

So the system's model is settled and unambiguous:

> **`base_commit` == `merge_base(upstream, branch)`** — the fork point the branch
> actually sits on.

Publish *verifies* this invariant. The mint *fails to establish* it. That is the
entire bug, and it is precisely the directive's "computed in two places": the
mint reads `origin/main`'s tip, publish computes `merge_base`. They agree only
when the branch happens to sit at `origin/main`'s tip — i.e. on a first mint.

### Why the pair diverges (the exact line)

In `ensure_working_pr_with_authority`:

- **3562** — `let (base_ref, base_commit) = resolve_upstream_base(...)` computes
  the base from the *current* `origin/main`, **unconditionally**.
- **3580** — `if current != branch { … checkout_new_branch_from(&worktree, &branch, &base_ref) … }`
  positions the branch at that base **only when the worktree is not already on it**.
- **3666** — records `base_commit` from the 3562 read regardless.

On a **first** mint the branch is created at `base_ref`, so the pair agrees. On a
**re-mint** the worktree is already on the successor branch (`current == branch`),
the whole checkout block is skipped, the branch never moves — and a base from a
*newer* main is recorded against it. **Incoherent by construction.**

The `current == branch` case is deliberate, not accidental: the doc at 3208-3211
calls it the partial-rotation adoption path ("worktree already on the next branch
is adopted, not refused"). Adopting the *branch* was designed; adopting its
*base* was never considered.

### The fix: establish the invariant publish already verifies

Compute the base from **the branch, after it is positioned**, using the same
expression the parity proof uses:

```rust
// After the `if current != branch { … }` block, before constructing TaskPr:
let base_commit = merge_base(&session.worktree, &base_ref, &branch)?;
```

`resolve_upstream_base` still supplies `base_ref` (and its fetch); only the
*recorded commit* changes authority — from a parallel read of `origin/main` to
the branch's true fork point.

Why this is correct in every case, not just W2-300's:

| Case | `merge_base(origin/main, branch)` | Cut reads | Correct? |
|---|---|---|---|
| Fresh branch cut at `origin/main` tip | that tip — **identical to today** | `ProvenEmpty` | ✓ no behaviour change |
| Fresh branch + cherry-picked carry | still the tip (carry are new commits on top) | `Range` | ✓ carry is real work |
| **Re-mint, empty branch at old main B1** | **B1 = branch tip** | **`ProvenEmpty`** | ✓ **fixes W2-300** |
| Re-mint, branch carries work on B1 | B1 | `Range` | ✓ work **not** marked empty |
| Branch shares no history | `Err` → fail closed at mint | — | ✓ no incoherent row written |

A merge-base is **always** an ancestor of both inputs, so
`is_ancestor(base, branch)` can never be false. The incoherent pair becomes
**unrepresentable** rather than merely discouraged — a shape, not a rule a caller
must remember (W2-304's lesson: *when asked to order two side effects correctly,
find the primitive that makes the wrong order impossible*).

Verified against the live incident before writing a line of code:

```
$ git merge-base origin/main jack-heart/do-not-complete-a-task-2
9f6bdd4986c29a111726758701d7729781680e4a   # == branch tip -> ProvenEmpty
```

### The live repair: heal on adopt, using the primitive that already exists

The mint fix alone does **not** free W2-300, and this must be said plainly rather
than assumed. Sequence 2's row already exists with the bad base, and nothing
re-mints it: `ensure_working_pr_with_authority` returns the **active** PR early
(3502-3508), and W2-304's discard only runs inside a terminal write the gate will
never authorise. The row is permanently incoherent. Deadlock.

Abandoning it is forbidden (abandoning a rotation artifact mints its replacement
— ENG-7) and store surgery is forbidden. The only remaining path is to make the
recorded base truthful, and **the primitive for that already ships**:
`heal_task_pr_base` (`store/sqlite/child_sessions.rs:3927`), whose own doc says
it exists to move the "otherwise-immutable `base_commit`… keyed on the row's true
identity". The publish path already calls it (`ops/task.rs:1518-1523`) for the
opposite drift direction (`B < M`, base stale behind the fork point). W2-300 is
the `M < B` direction, which no caller heals today.

So: when `ensure_working_pr_with_authority` **adopts** an existing active
unpublished Working PR, apply the same one-authority rule to it.

```rust
if let Some(active) = store.active_task_pr(&session.id).await? {
    return Ok(Some(heal_incoherent_base(store, session, active, lease).await?));
}
```

- Detection is **local and free**: `is_ancestor(pr.base_commit, pr.branch)` — no
  fetch. A coherent row (every healthy Task, every iteration) returns untouched.
- Only an already-broken row pays a fetch, then heals to
  `merge_base(base_ref, branch)` under lease authority.

This is one rule (`base == merge_base(upstream, branch)`), one authority, applied
at both the sites that write a base. It does **not** loosen `Unprovable`: the
gate still fail-closes on every tri-state it is given. It repairs the *data* the
gate reads, replacing a value the mint fabricated with the fork point the branch
genuinely sits on. There is no case where it converts real work into "empty" —
the range is computed from the true fork point, so carried commits still read
`Range`.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Is the directive's premise true? | Verified in the live store + git: base `3e9df0677`, branch `9f6bdd498`, `is_ancestor` false, base is a *descendant*. | Proceed; premise exact. |
| Which code path re-mints? | `current == branch` (3580) skips the checkout. Confirmed empirically: W2-300's worktree **is** on `…-2` at `9f6bdd498`. | Fix targets the base read, not the checkout. |
| Does `merge_base` fix the live case? | `merge_base(origin/main, …-2)` = `9f6bdd498` = branch tip → `ProvenEmpty`. Ran it. | Fix proven against the real incident. |
| Would it fail-open on carried work? | No — cherry-picks are new commits atop the fork point, so `merge_base` is unchanged and the range still reads `Range`. | Non-destructive; `Unprovable` rule untouched. |
| Is `merge_base` an ancestor by construction? | Yes, always. Verified on the live pair (`coherent: true`). | Incoherence becomes unrepresentable. |
| Does a primitive already exist? (memory: check the **store** layer) | **Yes** — `heal_task_pr_base`, already called by publish for the opposite drift. | Reuse it; write no parallel mechanism. |
| Does the mint fix alone free W2-300? | **No.** Active PR returns early; the discard needs a terminal write the gate blocks. Read, not assumed. | Live heal is required, and stated honestly. |
| Do open PRs touch this? | #1058 and #1052 touch `ops/task.rs` but neither touches the base computation or `ensure_working_pr_with_authority`'s mint. #1041 is scratch-only. | No duplicate work. |
| Is `merge_base` available? | `engine/git.rs:134`, already used at `ops/task.rs:1485`. Needs adding to the import list at line 15-19. | No new dependency. |
| Would resetting the branch work instead? | It destroys cherry-picked carry in the crash-recovery case. | Rejected — see alternatives. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| **Reset the reused branch to the newly recorded base** (directive's option A) | Makes the pair agree, but `git reset --hard` on adopt destroys any cherry-picked follow-up a partial rotation already applied. | Fail-destructive. The `current == branch` path exists *because* a prior rotation may have carried work onto the branch. Silently discarding merged-adjacent follow-up is worse than the loop. |
| **Record `base = rev_parse(branch)`** (naive "base the branch sits at") | Trivially coherent. | **Fail-open.** A branch carrying real work would record base == tip → `ProvenEmpty` → the gate discards a successor holding committed work. Exactly the hole #1050 closed. |
| **Loosen the `Unprovable` rule** | Frees W2-300 immediately. | Explicitly forbidden, and correctly so — reopens #1050's fail-open hole. The data is wrong, not the rule. |
| **Read `base_commit` before positioning, keep `origin/main`, and re-cut the branch only when empty** | Preserves carry, fixes W2-300. | Two authorities and a conditional — the exact "computed in two places" shape the directive forbids. `merge_base` subsumes it in one expression. |
| **Fix at publish only** (extend the `M < B` arm to heal) | Reuses the existing heal call site. | Wrong path: W2-300's successor must be **discarded**, never published. The gate, not publish, is where it is stuck. |
| **Heal inside `reconcile_task_pr_with_authority`** | Runs on every status read, so it repairs sooner. | Mutates the store on a **read** path. W2-304 deliberately kept the gate pure so `task_status` stays safe to call. Heal belongs in the write path that owns the lease. |

## Key decisions

**One authority: `merge_base(upstream, branch)`.** Not a new invention — the
publish parity proof already treats it as the definition of `base_commit`. The
mint is brought into line with the check that already exists. Both writers of a
base now compute it identically.

**Compute the base *after* the branch is positioned.** The ordering is the fix.
Reading `origin/main` before deciding where the branch sits is what lets the two
drift apart.

**Correctness by shape, not by discipline.** A merge-base is always an ancestor,
so the gate's ancestry check cannot fail for incoherence. No caller has to
remember anything.

**Include the live heal, and say why it exceeds the literal "Done when".** The
seed's Done-When covers only the mint. I am adding the adopt-time heal because
without it the reported incident stays broken, the fix is unprovable in
production, and both sanctioned escapes (abandon, surgery) are forbidden. It is
~10 lines, reuses a shipped primitive, and applies the identical rule. Flagged
explicitly so a reviewer can cut it — see `scratch/questions.md`.

**Detection before fetch.** Incoherence is detected with a local `is_ancestor`;
only a broken row pays the network cost. Healthy Tasks are untouched.

## Scope

**In scope**
- `ensure_working_pr_with_authority`: record `base_commit = merge_base(base_ref, branch)`
  after the branch is positioned.
- Adopt-time heal of an active unpublished Working PR whose base is not an
  ancestor of its branch, via the existing `heal_task_pr_base_for_lease`.
- Regression proving a **re-mint** (not a first mint) yields `ProvenEmpty`.
- Import `merge_base` in `ops/task.rs`.

**Out of scope**
- The `Unprovable` tri-state rule (#1050) — untouched.
- W2-304's discardable-successor settlement (#1056) — untouched.
- The publish-path `M < B` "contaminated" message, which misreports this case
  (it says the base "carries commits not on origin/main" when the base *is*
  origin/main). Real but separate; noted in `scratch/questions.md`.
- Store surgery on W2-300, and abandoning its successor.

## Done when

1. **Regression fails on today's code.** In `ops/task.rs` tests: mint a successor
   at base `B1`, advance `origin/main` to `B2`, force a **re-mint** (worktree left
   on the successor branch, row discarded — mirroring the live sequence), assert
   the cut reads `ProvenEmpty` (not `Unprovable`), the Task completes **exactly
   once**, and **no replacement row** is minted.
2. **Sabotage proof.** Reverting the mint to `resolve_upstream_base`'s
   `base_commit` turns the regression red. A first-mint-only test passes with the
   bug fully present — that is what hid this from both W2-300 and W2-304 — so the
   test **must** re-mint after main moves. The suite must contain a test that
   dies when the fix is removed.
3. **Coherence invariant.** For every minted PR,
   `is_ancestor(base_commit, branch)` holds by construction.
4. `cargo fmt`, `cargo clippy --all-targets -- -D warnings` (not `--lib` alone —
   test-target lints escape a lib-only run), `cargo test -p loopflow --lib ops::task`.
5. **Live evidence.** W2-300's cut reads `ProvenEmpty` under the fixed binary and
   it completes exactly once, staying completed. Reported with the command and
   output, not asserted.

## Measure

Baseline, now: W2-300 `status=waiting`, sequence 2 `base=3e9df0677` vs branch
`9f6bdd498`, `is_ancestor` false, cut `Unprovable`, reopen-and-re-mint per cycle
— unbounded bodies, never converging.

After: cut `ProvenEmpty`, one completion, zero replacement rows, zero reopens.

```bash
sqlite3 ~/.lf/loopflow.db "SELECT p.sequence, p.base_commit FROM task_prs p \
  JOIN task_sessions s ON p.task_session_id=s.id WHERE s.issue_identifier='W2-300';"
git merge-base --is-ancestor <base> <branch> && echo coherent
```

Fleet-wide: count active unpublished Working PRs whose recorded base is not an
ancestor of their branch. Target zero, and — unlike ENG-4's `revoked=0` measure,
which silently assumed a sweeper the design refused — this one has a driver: the
mint can no longer create such a row, and adopt-time heal removes the existing
ones. The count is the right instrument **because** a code path drives it.
