# The running lf reports its own staleness

## Problem

A merged fix that never reaches `/Users/jack/.local/bin/lf` is invisible. Nothing
on any surface says the running fleet is older than main, so merged code keeps
producing zero fleet behavior while everyone reads green CI and assumes the fix
is live.

Measured cost (2026-07-17, from the Task seed and the live store): two Tasks
abandoned holding merged work (ENG-7, W2-285), seven body generations burned on
W2-290, and 29 scratch-clear-only `ci_incidents` — 14 of which armed a ci-fix
command that could not succeed — across 14 Tasks and 17 PRs in a 10.5-hour
window, reaching outside this Project (PRD-5). Every one of those bodies is
remedied by code already on main.

The defect is not the missing rebuild. It is that **nothing observes the gap**.
Wave memory now carries per-fix workarounds ("check `strings lf | grep -c
'verifiably absent'` before trusting either rule") because doctrine depends on
which binary is running. That is doctrine paying rent for a missing check.

## The finding that shapes this design

**The answer is already inside the binary. Nothing prints it.**

`build.rs:149` has emitted `LOOPFLOW_BUILD_SOURCE_REVISION` — the git HEAD sha at
build time, `-dirty`-suffixed when the tree was dirty — since #962 (2026-07-15).
`build_info::source_revision()` returns it. Its only consumer is the
`schema_migrations.applied_by` row (`migrations.rs:759`). No human- or
machine-facing surface reads it.

Verified on this host, not inferred:

```
$ strings /Users/jack/.local/bin/lf | grep -c 3e9df06777297cd1cb83fccc1d0261fd3e74dfa8
1
$ strings /Users/jack/.local/bin/lf | grep -m1 3e9df0677... | cut -c1-160
...VALUES (?1, unixepoch())release3e9df06777297cd1cb83fccc1
```

That `release` immediately followed by the sha is `LOOPFLOW_BUILD_SOURCE_ROOT`
and `LOOPFLOW_BUILD_SOURCE_REVISION` packed adjacent in `.rodata`, exactly as
`build.rs` emits them. The installed fleet binary was built from **`3e9df0677`**
(#1053, merged 08:02:42Z), and:

```
$ git merge-base --is-ancestor 3e9df0677 origin/main   # exit 0 — cleanly behind
$ git rev-list --count 3e9df0677..origin/main
4
```

The four merged commits the running fleet does not have:

| sha | merged | subject |
| -- | -- | -- |
| `f19f41aab` | 08:51:55Z | task actions: recommend only actions the lifecycle can execute (#1041) |
| `6232569e4` | 09:09:31Z | task: scope required reviews to active epochs (#1060) |
| `15e441e69` | 09:46:28Z | ci-fix wake: refuse to arm on a land-time precondition (#1062) |
| `062bd1e4c` | 10:32:09Z | task: settle a bounded ci-fix turn above the parent lifecycle loop (#1063) |

So the seed's hand-measured "binary mtime 09:00:12Z, #1060 merged 09:14" was
directionally right and **understated the gap**: mtime says 09:00Z, but the code
in it is from 08:02Z, so `f19f41aab` (merged 08:51Z, before the build) is also
missing. mtime measures when the linker ran, not what it compiled. That is the
proxy failing in the direction that hurts — it reports the fleet fresher than it
is.

This design ships the `println`.

## The demo

On this host, today:

```
$ lf doctor
binary   release 0.11.3 built from 3e9df0677
         4 merged commits behind origin/main (062bd1e4c):
           f19f41aab  task actions: recommend only actions the lifecycle can execute (#1041)
           6232569e4  task: scope required reviews to active epochs (#1060)
           15e441e69  ci-fix wake: refuse to arm on a land-time precondition (#1062)
           062bd1e4c  task: settle a bounded ci-fix turn above the parent lifecycle loop (#1063)
         a rebuild is an operator action; this check does not install anything
...
warn  binary-freshness  running lf is 4 merged commits behind origin/main
```

The supervisor who spent this session reasoning "which binary is running, and
does it have the fix?" runs one command and reads the answer. No `strings`, no
mtime arithmetic, no per-fix memory entry.

## Approach

Three changes, smallest first.

### 1. Report the stamp (`lf doctor` identity block)

`DoctorReport.store` (`doctor.rs:78`) already prints `build_provenance`,
`build_source_identity`, and `build_source_root` — and omits
`build_source_revision`, the only one that identifies the code. Add it. One
field, one line. This alone would have answered the question this session, three
times.

### 2. Classify the stamp against origin/main (the new primitive)

A pure classifier in `build_info`, taking the revision and a repo path so it is
testable against a temp git repo with no store:

```rust
/// Where a binary's build revision sits relative to merged main.
///
/// `Behind` is the only variant that means "a merged fix is not running".
/// Absence of proof is `Unprovable`, never `Current` — a binary that cannot
/// locate its own source is not thereby fresh.
pub enum BuildFreshness {
    Current { revision: String },
    Behind { revision: String, missing: Vec<MergedCommit> },
    OffMain { revision: String },
    Unprovable { reason: String },
}
```

Resolution order, each step failing to `Unprovable` with a named reason:

1. **Stamp usable?** `"unknown"` (built with no git root) or a `-dirty` suffix →
   `Unprovable`. A dirty build is not any commit.
2. **Repo?** `build_info::source_root()` when present and still on disk,
   otherwise walk up from cwd. No repo → `Unprovable`.
3. **Right repo?** `git cat-file -e <revision>^{commit}`. If the stamped object
   is not in this repo, we are standing in someone else's checkout →
   `Unprovable("stamp <sha> is not an object in <repo>")`. This is the
   is-this-loopflow test, self-validating and needing no config.
4. **Refresh** `origin/main` best-effort (`engine::git::fetch`, read-only, one
   ref). On failure, fall back to the local ref and carry a
   `compared against a local origin/main last updated <age> ago` caveat into the
   detail — never silently compare against a stale ref.
5. **Compare.** Equal to tip → `Current`. `is_ancestor(revision, origin/main)` →
   `Behind` with `git log revision..origin/main` as the missing list. Resolves
   but not an ancestor → `OffMain` (a dev build on a feature branch is not
   stale).

Step 3 is load-bearing and is why this is a tri-state rather than a bool.
`engine::git::is_ancestor` maps **every nonzero exit to `false`**
(`git.rs:128-133`), so `merge-base --is-ancestor` returning 128 for an unknown
object is indistinguishable from 1 for "not an ancestor". Calling `is_ancestor`
without first proving the object exists would silently classify a missing stamp
as `OffMain` and report a stale binary as a healthy dev build — fail-open, in
exactly the shape W2-300 already paid for. Proving object existence first makes
that conflation unrepresentable.

Status mapping: `Current`/`OffMain` → `Ok`, `Behind` → `Warn`, `Unprovable` →
`Warn` naming the reason. Never `Fail`: `doctor` exits non-zero on `Fail` and a
cron gate must not break because someone ran it outside a checkout.

### 3. Make the store record which sha booted each body

`BinaryProvenance` (`child_session.rs:241`) is stamped by each body at boot and
is how the seed knew "the fleet is on 0.11.3". It carries `version`,
`provenance`, `source_identity` — and not the revision. That is precisely why
`0.11.3` was the only available answer, and `0.11.3` has been the reported
version across every fix in evidence. Add:

```rust
/// The `LOOPFLOW_BUILD_SOURCE_REVISION` of the binary that booted this
/// generation. `None` for generations booted before this field existed, and for
/// a build with no git root — absent means unstamped, never "current".
pub source_revision: Option<String>,
```

`Option` matches the precedent one field up (`provenance: Option<BinaryProvenance>`,
"generations recorded before this field was added") and keeps historical rows
readable. Not a wire DTO: `BinaryProvenance` appears only in Rust
(`grep -rln BinaryProvenance` finds no Swift and no `tests/fixtures/dto` entry),
so the no-defaults DTO rule does not apply and no mirror needs updating.

**Scope change during implementation (was: a `fleet-freshness` doctor check).**
The design proposed a doctor check aggregating distinct revisions across live
sessions. Implementation found `format_child_body` (`bin/lf.rs:582`), which
already prints `binary 0.11.3 (release)` on every `lf task status` — **that is
the exact line the seed read to conclude "every live Task body reports binary
0.11.3"**. The revision belongs there, on the surface supervisors already use,
rather than in a second aggregate that answers the same question one command
away. `lf task status` now reads `binary 0.11.3 (release) from 3e9df0677`, or
`revision unstamped` for a body booted by a binary that carried no stamp.

That is a reduction, not a deferral: the aggregate check would have duplicated a
read that already exists, and until the fleet rebuilds it would have reported
every body as unstamped anyway.

## De-risking

| Question | Finding | Impact on design |
| -- | -- | -- |
| Does a build stamp exist at all, or must one be added? | It exists — `build.rs:149`, since #962 (2026-07-15). `build_info::source_revision()` reads it. | No build-system change. This is a reporting bug, not a missing capability. Grepping the store API/tree first (per MEMORY) saved building a parallel stamp. |
| Does the **release** build capture a real sha, or `"unknown"`? | Real. `install.py` builds in-repo (`cargo build -p loopflow --release` from `ROOT`, `_release_build_env` sets only `LOOPFLOW_BUILD_PROVENANCE=release`), and `release.yml` builds from an `actions/checkout`. Both have a git root. Only `source_root` is forced to `"release"`; the revision is untouched. Confirmed in the installed binary: `3e9df0677`. | The mechanism works today for the exact binary in evidence. No packaging change needed. |
| Is `strings` a viable comparison interface? | **No** — and the failure is instructive. `grep -oE '[0-9a-f]{40}'` over the binary yielded 86 candidates, **zero** resolving via `git cat-file -e`. Cause: the stamp is packed as `...release3e9df0677...`, and `e`/`a` are hex, so the hex run starts at the trailing `e` of `release` and every 40-char extraction is offset by one. A substring grep for a known sha finds it; a structural extraction does not. | Confirms the seed's warning from a new angle and justifies an explicit reported field. Also a live instance of MEMORY's "a 0 from a filter is only evidence if the producer ran" — the control probe (145,086 strings lines, `build_provenance` ×3) is what proved the probe was live and the zero was mine, not the binary's. |
| Is `lf --version` sufficient? | No. `0.11.3` across every fix in evidence, including builds four commits apart. The installed `lf auth --help` still lists `lf auth exec`, deleted by #1029. | The version string is not evidence about the tree. Compare the sha; report the version only as context. |
| Is mtime sufficient? | No, and it errs toward "fresher than reality": mtime 09:00:12Z vs. code from 08:02Z hides `f19f41aab` (merged 08:51Z). Also wrong across a rebuild that installs nothing new, and wrong if the clock moves. | Rejected as the comparison. Sha-vs-ancestry is exact. |
| Would `is_ancestor` alone be safe? | No. `git.rs:128-133` maps every nonzero exit to `false`, conflating "not an ancestor" (exit 1) with "unknown object" (exit 128). | Prove object existence with `cat-file -e` first; tri-state, fail-closed to `Unprovable`. |
| Can a stale local `origin/main` make the check silently under-report? | Yes — the exact failure mode being fixed. | Best-effort single-ref fetch; on failure, fall back **and name the caveat** in the detail. |
| Is `BinaryProvenance` mirrored in Swift/DTO fixtures? | No — Rust-only. | Field addition costs no mirror work; DTO no-defaults rule does not apply. |
| Is a `git fetch` inside `doctor` safe next to live bodies? | Yes. A fetch updates remote-tracking refs only; it touches neither the working tree nor `.git`'s sequencer. MEMORY's concurrency hazards are about rebase/reset, not fetch. | Fetch stays in the check; no new flag. |

## Alternatives considered

| Approach | Tradeoff | Why not |
| -- | -- | -- |
| Compare binary mtime against the newest merge timestamp | Zero code; it is what the operator did by hand | Understates the gap (proven above: misses `f19f41aab`), wrong across a no-op rebuild, wrong if the clock moves. A proxy for the fact when the fact itself is embedded. |
| Compare the reported version string | Trivial | `0.11.3` across every fix in evidence. Not evidence about the tree. |
| `strings` the binary for a marker literal | No code | Offset-by-one hex-run trap (above); needs a per-fix literal, which *is* the memory-rent this Task removes. |
| Have `lf` check GitHub for the latest release/tag | Works without a checkout | Answers "is there a newer tag", not "does the running code contain merged fix X". Fixes merge to main continuously; tags lag. Adds a network dependency and a token to a diagnostic. |
| Auto-update when stale | Closes the gap for real | **Explicitly out of scope.** A fleet rebuild mid-flight is an operator decision; putting it behind an agent is a larger, unapproved bet. The seed says to stop and say so if a self-updater appears. |
| Warn on every `lf` invocation | Impossible to miss | Noise on every command, and it would need the fetch on every command. `doctor` is the read; keep it a read. |

## Key decisions

**The signal is the deliverable; no installer.** The check reports and names the
operator action. It never rebuilds, installs, or restarts anything.

**Tri-state, fail-closed to `Unprovable`.** A binary that cannot locate its own
source is not thereby current. Absence is loud, never silently `Ok` — the
absence-in-one-projection rule applied write-side.

**`Unprovable` warns rather than staying quiet.** Running `doctor` outside a
checkout costs one honest warn line naming the reason. That is a shape, not a
rule someone must remember, and it is the cheapest way to keep silence from ever
meaning "fine".

**`OffMain` is `Ok`, not a warning.** A dev build on a feature branch is not
stale. Reporting merged commits since the merge-base for dev builds would warn on
every worktree; the measured cost is the release fleet. Noted as an open
question rather than designed in.

**The contract lives in rustdoc, not here.** `lf pr land` clears `scratch/` as
its first act, so this doc cannot be the interface. The `BuildFreshness` doc
comment ("`Behind` is the only variant that means a merged fix is not running";
absence is `Unprovable`) is the durable surface. Design sha gets tagged before
land (`git tag no-surface-reports-that-the-design <sha>`), as W2-294 did.

**Not `lf status`.** Defensible per the seed, but one supported read satisfies
the Done-when. `doctor` is the diagnostic home and already carries the identity
block. Adding a second surface doubles the fetch question for no new answer.

## Scope

**In scope**

- `build_source_revision` in `lf doctor`'s identity block.
- `BuildFreshness` classifier in `build_info` + `binary-freshness` doctor check.
- `source_revision: Option<String>` on `BinaryProvenance`, stamped at boot.
- The revision on `lf task status`'s body line (`format_child_body`), replacing
  the planned aggregate fleet check — see the scope change above.
- Rustdoc contract on `BuildFreshness`.

**Out of scope**

- Any auto-update, auto-rebuild, or install. Not this Task's decision to make.
- Fixing W2-290. It needs a rebuild (an operator action); its exit already exists
  in merged code (#1060 drops the epoch-3 bar; #1051 converts its policy so the
  next gate mints `defer`). This Task is about seeing the gap.
- `lf status` warning surface.
- Warning dev builds about merged commits since their merge-base.

## Done when

1. `lf doctor` on this host prints the build revision, states the running binary
   is behind `origin/main`, and lists the missing merged commits by sha and
   subject. Today that output names `3e9df0677` and the four commits above.
2. `lf doctor --json` carries `store.build_source_revision` and a
   `binary-freshness` check whose detail names the count and the missing shas.
3. A binary built at `origin/main`'s tip reports `Ok` and lists nothing.
4. `lf doctor` outside a loopflow checkout reports `Warn` naming the reason, and
   never reports `Ok`.
5. No install/rebuild/restart path is added: `git diff origin/main...HEAD` shows
   no new process spawn in the freshness path.

### Regressions, each with its sabotage

Fixtures build a temp git repo (commits `A → B → C`, `origin/main` at `C`) and
call the classifier with a revision — no store, matching `doctor.rs`'s
"checks are pure functions of the rows, tested without a store".

| Test | Sabotage that must turn it red |
| -- | -- |
| stamp at `A` → `Behind{missing:[B,C]}`, subjects in order | make the classifier always return `Current` |
| stamp at `C` → `Current`, empty missing list | make it always return `Behind` |
| stamp is a sha absent from the repo → `Unprovable`, **not** `OffMain` | drop the `cat-file -e` guard and call `is_ancestor` directly — this is the fail-open shape, and the test exists to keep it dead |
| stamp `"unknown"` → `Unprovable` | accept `"unknown"` as a revision |
| stamp `"<sha>-dirty"` → `Unprovable` | strip the suffix and compare the base sha |
| stamp on a branch off `B`, not an ancestor of `origin/main` → `OffMain` | collapse `OffMain` into `Behind` |
| `BinaryProvenance::current()` carries a non-empty `source_revision` in a git build | hardcode `source_revision: None` |
| a generation JSON with no `source_revision` still deserializes, yielding `None` | make the field required |

The pairing that matters is rows 1+2: a classifier hardcoded to either verdict
kills exactly one of them. A fixture asserting only "some verdict was returned"
would pass both, which is pinning the fixture.

## Verification status

**Local proof is unavailable on this host, and hosted CI is the verifier.** Said
plainly rather than narrated as an expected result.

Measured, not assumed: `cargo check -p loopflow --all-targets` progressed through
~40 dependencies and then wedged, with three build-script executables
(`quote`, `proc-macro2`, `libc`) sitting in state `S` for 12 minutes with no
target-directory growth. Running one directly confirms the cause:

```
$ target/debug/build/libc-*/build-script-build
STILL RUNNING after 10s -> stalled before main
```

That is filed issue 47880291 — `syspolicyd` stalls newly linked binaries before
`main`; it is pegged at 100% CPU with 488 minutes accumulated. `cargo check`
cannot proceed because it must *execute* build scripts, so this blocks type
checking, not merely test execution.

Two corroborations from the same measurement, both worth acting on separately:

- **The fleet rebuild is wedged by the same defect.** `scripts/install.py refresh`
  (pid 75307) has been running 48 minutes holding a cargo that cannot finish. So
  the operator action this Task's signal points to is *currently impossible on
  this host*, which raises 47880291's priority: the staleness cannot be closed
  until it is fixed. This check would at least make the resulting gap legible.
- **A killed producer's exit code is not evidence.** The backgrounded
  `cargo check ... | tail -40` reported exit 0 after I killed cargo — that is
  `tail`'s status. Same family as the `grep -c` traps already in MEMORY: the
  consumer succeeded, the producer never did.

Neither sibling cargo was touched (pids 42954 and 76155 both had live parents in
other worktrees); only this worktree's own process tree was killed.

## Measure

Baseline, measured today and reproducible from the seed's query:

```sql
select failure_set_json, trigger_command_id, responded_at
from ci_incidents where failure_set_json='["scratch-clear"]';
```

29 scratch-clear-only incidents / 14 armed / 11 with a body spending a turn, in
10.5 hours. Fleet gap at design time: 4 merged commits, invisible from every
machine surface.

After: the gap is reportable in one command. The honest measure is **not** that
the number of stale-binary incidents falls — this Task installs nothing, so it
cannot move that number, and claiming it would be the "Measure names a count no
mechanism drives" error MEMORY already records against ENG-4. The measure is:

- `lf doctor` names the gap on a host whose binary predates `origin/main` (proven
  by the fixture pair, and demonstrable today).
- Wave memory sheds its per-fix binary-probe workarounds. The entry "check
  `strings lf | grep -c 'verifiably absent'` before trusting either rule" is
  replaced by one command. Deleting that rent is the outcome.

## Open questions

- Should a dev build report merged commits since its merge-base? Useful for a
  body running a stale worktree build; noisy for every feature branch. Left
  `OffMain`/`Ok`; revisit if a stale dev build ever causes a measured incident.
- `doctor` fetches `origin/main` best-effort. If a future cron runs `doctor` on a
  network-isolated host, the fallback caveat covers correctness but the fetch
  attempt costs a timeout. Revisit if measured.
