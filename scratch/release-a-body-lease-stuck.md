# Release a body lease stuck at `revoked` when its process group is gone

## Problem

A body lease has one job: guarantee that no second body runs for a Session. The
lease is taken at reservation and released when the body is reaped.

`reap_revoked_child_body` (`ops/child.rs:66`) does both halves in sequence:

```rust
crate::engine::process::reap_child_process(&revoked, Duration::from_secs(2))
    .await
    .map_err(child_error)?;          // <- early return
store.finish_revoked_task_process(session_id, revoked.generation).await
```

When the kill fails, the `?` returns and `finish_revoked_task_process` never
runs. The lease stays `revoked` forever, and `reserve_task_process`
(`store/sqlite/child_sessions.rs:669`) CASes on

```sql
AND (process_lease_state IS NULL OR process_lease_state = 'finished')
```

which `revoked` satisfies on neither branch. Every later generation is refused.
This is not a race; nothing in the system ever re-examines that lease.

The live instance is W2-230: `process_generation=2`,
`process_lease_state='revoked'`, `status=waiting`, and a durable
`status_reason` (written at `ops/task.rs:2405`) ending *"manual cleanup is
required"* — the third human-instruction-with-no-reader in this subsystem,
alongside `ops/task.rs:1806` (fixed in W2-267) and `ops/child.rs:365`. The
parent Project Session checked: process group 27084 has no members. The lease is
pinned by a corpse.

Scale: 1 of 117 Sessions; lease states across the registry are active=41,
finished=75, revoked=1. Rare, permanent, and unaided — which is exactly the
Developer Efficiency KR *"No Task strands on a dead body: across one full week
of real runs, zero Sessions sit in failed awaiting a manual resume."*

## The demo

On a Session whose lease is stuck at `revoked` and whose process group is gone,
`lf task resume <issue>` starts the next generation instead of failing. The
event log shows one `BodyLeaseChanged` to `finished` naming the absence that
justified it, and no human touched the store. On a Session whose group is still
alive, the same command refuses and names the live lease.

## Approach

Split the two halves of the reap that are currently fused, and make the second
half — releasing the lease — depend on evidence rather than on the kill having
succeeded.

**The claim the design rests on:** the lease exists to bar a second body. A
process group that is verifiably absent cannot run anything. So an absent group
must not block a reservation, whatever happened during the reap that was
supposed to clear it. A group that still exists — or whose absence cannot be
proven — keeps the lease, because there the lease may be doing its actual job.

### 1. An absence probe that only reads

New in `engine/process.rs`:

```rust
pub(crate) enum BodyPresence {
    Gone,        // every recorded identity is authoritatively absent
    Present,     // something answers to a recorded identity
    Unprovable,  // the host could not be asked
}

pub(crate) async fn probe_child_body_presence(
    process: &ChildProcessGeneration,
) -> BodyPresence
```

It probes, in order: the recorded tmux session, the recorded process group, the
recorded pid. `Gone` requires *every* identity to be **positively** absent. Any
identity `Present` makes the body `Present`; otherwise `Unprovable`. The probe
never guesses, and — the rule the whole design turns on — **it never treats an
unanswered question as a "no".**

Absence per signal identity is a tri-state `kill(target, 0)`:

| outcome | meaning | verdict |
|---------|---------|---------|
| `ESRCH` | no process answers | `Absent` — authoritative |
| returns `0` | a process answers | `Present` |
| `EPERM` | a process answers, we may not signal it | `Present` |
| any other errno | the kernel refused the question | `Unprovable` |
| id does not fit `i32`, or converts to `0` | not a body identity we can ask about | `Unprovable` |

`EPERM` reads as **present**, and that is not conservatism for its own sake —
POSIX only returns `EPERM` for `kill(-pgid, …)` when the group had at least one
member the caller lacked permission for. On a recycled pgid it means *someone
else's* process is there. We cannot distinguish that from our own body, so we
refuse to release. This is where W2-230's original reap failure sat.

**This probe is new; it does not reuse `process_target_exists`.** That function
is a `bool`, and its two false-y collapses are safe where it lives and unsafe
here:

```rust
Group(group) => i32::try_from(group).map_or(0, |group| -group),  // out of range -> 0
if raw == 0 { return false; }                                    // ...which reads as absent
result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
//                                                    ^ ESRCH and every other errno -> false
```

For `wait_for_process_exit`, `false` means "stop waiting" and both collapses are
harmless. As a *release* predicate they are fail-open: an out-of-range recorded
group id, or an `EINVAL`-class refusal, would read as `Gone` and release a lease
over a body nobody ever asked about. The two callers want different things from
the same syscall — a bounded waiter wants "is it still there?", a release wants
"can I prove it is not?" — so they get two functions. `wait_for_process_exit`
keeps its bool and is untouched.

Note that `raw == 0` is not merely unrepresentable: `kill(0, sig)` signals the
*caller's own* process group. Reading it as absence is the worst available
answer, which is why it is `Unprovable` rather than folded into `Absent`.

The tmux identity gets the same treatment, and for the same reason — the two
existing helpers are both fail-open at exactly the point a release cares about:

```rust
// tmux_session_exists: any non-zero exit -> Ok(false) == "no session"
Ok(status.success())
// tmux_live_sessions: any failure -> empty set == "nothing is live"
if !output.status.success() { return Ok(std::collections::HashSet::new()); }
```

A tmux server that errors, and a tmux server that reports no such session, are
the same value. So the release probes tmux with its own tri-state, over
`tmux list-sessions -F '#{session_name}'` with stderr captured:

| outcome | verdict |
|---------|---------|
| exit 0, recorded name in the set | `Present` |
| exit 0, recorded name not in the set | `Absent` — the server answered authoritatively |
| non-zero exit, stderr recognized as no-server / no-sessions | `Absent` |
| non-zero exit, stderr unrecognized | `Unprovable` |
| the command cannot be spawned (incl. tmux absent from `PATH`) | `Unprovable` |

**"tmux binary missing ⇒ identity absent" is removed from this design.** It was
wrong twice over. `PATH` is per-process, so a probe running under a different
environment than the launcher can fail to find a tmux that is running fine — the
conclusion "no tmux here" is a fact about *this process's* `PATH`, not about the
body. And it is the same absence-in-one-projection error the wave has now paid
for repeatedly: a surface that cannot answer is not the world saying no.

`reap_child_process`'s `tmux_installed()` shortcut stays as it is. Reap and
release have opposite risk postures and must not share a predicate: a reap that
wrongly concludes "nothing to kill" moves on and the lease still gets released
by evidence later, whereas a release that wrongly concludes "nothing is there"
has already unbarred a second body. Reap fails open by design; release fails
closed by design. That divergence is deliberate and is worth a comment at both
sites.

Matching tmux's stderr by string is the one soft spot here, and it is soft in
the safe direction: if tmux rewords its no-server message, unrecognized stderr
falls to `Unprovable`, the lease stays blocked, and the behavior is exactly
today's status quo. There is no wording change that turns into a wrong release.
Note also that the common case — a running tmux server — is answered by exit 0
and set membership, and never reads the strings at all.

### 2. Release is probe-only. It never re-signals.

```rust
pub(crate) async fn release_dead_revoked_child_body(
    store: &SharedStore,
    target: &ChildRef,
    revoked: &ChildProcessGeneration,
) -> OpsResult<Option<ChildProcessGeneration>>
```

`Some(finished)` when the probe says `Gone` and the store's CAS
(`finish_revoked_*_process`, which itself CASes on `process_lease_state =
'revoked'`) accepts. `None` when the body is `Present` or `Unprovable`.

It does **not** retry the kill. Retrying a reap minutes or hours after the
generation died means signalling a pid/pgid that the kernel may have recycled
into an unrelated process — the release path would become a way to kill a
stranger's work. Probe, and if the answer is `Gone` there is nothing left to
kill anyway.

### 3. Three call sites

**(a) The reap failure path** — `reap_revoked_child_body` stops swallowing the
lease. On a failed `reap_child_process` it consults the probe; if the body is
gone, it finishes the lease and returns the finished generation, otherwise it
returns the original error unchanged. This closes the window where the group
dies between the failed kill and the check, and it needs no new opinion about
*why* the reap failed: "refusing to reap current process group" self-classifies
as `Present`, because that group is us.

**(b) The reservation boundary** — `launch_task_process` (`ops/task.rs:1842`)
and its Project twin attempt the release when the recorded lease is `revoked`,
before reserving. This is what makes the demo work: `lf task resume` reaches
`launch_task_process`, the dead lease clears, and the CAS passes. A lease that
cannot be released fails the command with the real cause instead of a CAS miss.

**(c) The stranded-recovery pass** — `recover_stranded_task_body`
(`ops/task.rs:2213`) attempts the release before `plan_stranded_recovery`.
This needs no new plan variant: once the lease is `finished`, the existing
verdict table reads `Finished` + `Superseded`/`Lost` and returns `Redispatch`
on its own. `strand_verdict`'s `Revoked` arm stays as W2-267 wrote it — it is
still exactly right for the lease that could *not* be released, and its comment
("Finishing such a lease — verifying the process group is truly gone, then
releasing it — is the reaping task's job") names this task. Only its final
clause is updated, since the release is no longer someone else's future.

### 4. The CAS message stops blaming status

`ops/task.rs:1882` reports every reservation miss as

> task W2-230 changed from waiting to waiting during process reservation; retry
> the command

The status did not change; the lease is what missed, and "retry the command"
sends a machine into a loop that cannot terminate. The fallback now reads the
re-read Session's lease first and names it:

> task W2-230 holds a `revoked` lease on body generation 2; a new generation
> cannot be reserved until that lease is released

The status-changed message survives for the case it actually describes.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Is the reap genuinely un-retried, or does something re-enter it? | Nothing does. `reap_revoked_child_body` has three callers (`revoke_and_reap_child_body`, the stall path at `ops/task.rs:2396`, a test), all of which reach it only while revoking a *fresh* generation. A lease already at `revoked` is never revisited. | The permanence is structural, not a lost wakeup. The fix must add a re-entry point, not repair a scheduler. |
| Would simply retrying `reap_child_process` fix it? | Yes, mechanically: `signal_process_target` maps `ESRCH` to `Ok(())`, so a retry against a now-empty group walks straight through TERM → `wait_for_process_exit` → `Ok` → finish. | Rejected anyway — a retry *signals*. Between the strand and the retry the kernel may have recycled the pgid, and TERM to a recycled group kills a stranger. Probe-only gets the same release with no blast radius. |
| Can `kill(-pgid, 0)` distinguish absent from unreachable? | Yes. `ESRCH` ⇒ no member exists; `EPERM` ⇒ a member exists that we may not signal. POSIX is explicit that `EPERM` on a negative pid requires at least one receiving process. | The three-way `Gone`/`Present`/`Unprovable` split is readable off one syscall. No `ps` scan, no `/proc` walk, no platform branch. |
| Can the existing `process_target_exists` be the release predicate? | **No.** It returns `bool`, and collapses two distinct things into `false`: an id that does not fit `i32` (`map_or(0, …)` → `raw == 0` → `false`) and *every* errno other than `EPERM` — so `ESRCH` and "the kernel refused the question" are the same value. | The probe is a new tri-state function; `process_target_exists` and `wait_for_process_exit` keep their bool and are untouched. Reusing it would have made the release fail **open** — releasing a lease on an unreadable identity is precisely the outcome this task exists to prevent. |
| Is "tmux is not on `PATH`" evidence that the tmux body is gone? | No. `PATH` is per-process: a probe with a different environment than the launcher can miss a tmux that is running. `tmux_session_exists` also maps *any* non-zero exit to `Ok(false)`, and `tmux_live_sessions` maps any failure to an empty set — a server error and an absent session are indistinguishable. | The release gets its own tri-state tmux probe. Only exit-0 set membership, or a positively recognized no-server/no-sessions stderr, counts as `Absent`; an unspawnable or unrecognized tmux is `Unprovable`. `reap_child_process` keeps its `tmux_installed()` shortcut — reap may fail open, release must fail closed. |
| Why did W2-230's kill return `EPERM` if `ps` showed no members? | The two observations are minutes apart, so they do not conflict: `EPERM` proves the group had a member at kill time; `ps` proves it had none later. Most likely a recycled pgid briefly owned by a process we could not signal. | Reinforces probe-at-release-time rather than reasoning from the recorded failure. The `status_reason` prose is a historical artifact and must never be parsed. |
| Does the design have a single fail-open path left? | No, by construction: `Gone` is a conjunction of positive absences, so every "I could not ask" anywhere in the probe demotes the verdict to `Unprovable` and the lease stays blocked. | The cost of every `Unprovable` is exactly today's behavior — a lease that stays `revoked` — so a wrong `Unprovable` costs nothing new, while a wrong `Gone` unbars a second body. The asymmetry decides every ambiguous case in the design. |
| Does releasing a lease on a `waiting` Session restart delivered work? | No. Releasing changes only the lease; status, PR phase, and the W2-129 open-PR bar are untouched, and `plan_stranded_recovery` returns `LeaveAlone` for `Waiting`/`Blocked` before the lease is ever read. | W2-230 (status=waiting) is released only when something explicitly asks to reserve — i.e. `lf task resume`. Automatic redispatch stays scoped to `Active`/`Failed` intents, exactly as W2-267 drew it. |
| Can this run against the live registry? | No, and it must not. Release 0.11.3 runs the fleet and dev builds are walled off from the production store (wave memory, 2026-07-17). | Every test builds its own store. The W2-230 row is evidence, not a target; the fix reaches it when a release ships. |
| Do the `ops::task` lib tests pass in a Task Session? | Two (`recover_refuses_a_non_abandoned_task`, `recover_abandoned_task_adopts_existing_worktree_pr_and_direction`) fail on clean main from inside a Session — they lack the `AMBIENT_TASK_ENV` scrub. Known, pre-existing (wave memory, 2026-07-16). | Not caused by this branch. New tests use the guard that scrubs ambient `LF_*`. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Widen the reserve CAS to accept `revoked` | One-line change, fixes every stuck lease at once | Deletes the lease. `revoked` means "a body may still be running" — accepting it lets a second body launch over a live one. The bug is the missing release, not the CAS. |
| Retry `reap_child_process` at the reservation boundary | Reuses the whole existing path, no new probe | Signals a possibly-recycled pid/pgid, hours later. Trades a stuck lease for killing an unrelated process. |
| A background reaper sweeping `revoked` leases on resident boot | Heals W2-230 with no command at all; neighbour of #1000 | A sweeper decides on its own schedule and needs its own liveness opinion, ownership story, and idempotency proof. Release-at-reservation heals precisely when it matters, with the caller present to hear the refusal. If a sweeper is wanted later it composes over this same probe. |
| `ps -A -o pgid=` to enumerate group members | Matches how the Project Session diagnosed W2-230 by hand | A fork per probe, and `ps -g` means different things on macOS and Linux. `kill(…, 0)` answers the same question with one syscall and no parsing. |
| Add a `StrandedPlan::ReleaseLease` variant | Makes the release explicit in the plan | The plan is pure and the release is I/O. Releasing *before* planning lets the existing verdict table read the released lease correctly with no new variant — less code, one authority. |
| An `lf task release-lease` escape hatch | Gives an operator a lever today | A third human-instruction-with-no-reader, one layer up. The done-when is "without a human". |

## Key decisions

**An absent group blocks nothing.** The lease's meaning is "no second body".
Absence discharges that meaning completely, so a lease pinned by a corpse has no
claim on anything. This is the answer to the task's open question ("consider
whether an unreapable-but-already-dead group should block anything at all"): it
should not, and the fix is not an override — it is the lease being read for what
it means.

**Probe, never re-signal.** The one thing the release path must not do is kill.
A stuck lease is old by construction, and a pid is a recycled resource.

**`EPERM` means present.** The rarer, more frustrating outcome — a Session
blocked on a group we cannot inspect — is the correct one. Releasing on `EPERM`
would release exactly the case where a body may still be alive and unkillable.
W2-230 stays stuck under this rule *at the moment of its reap failure*, and is
released later when the group is verifiably gone. That is the intended shape.

**Only a positive "no" counts as absence.** Not a `bool` that ran out of ways to
say yes, not a syscall that failed for a reason we did not model, not a tmux we
could not run. `Gone` is a conjunction of positive absences; everything else is
`Unprovable` and the lease holds. The two existing helpers this design first
reached for — `process_target_exists` and `tmux_session_exists` — are both
fail-open at exactly this point, which is safe for the bounded waiter and the
reaper that use them and unsafe for a release. So the release owns its own
probe, and the reaper keeps its shortcuts. **Reap may fail open; release must
fail closed.** Two functions, because they are asking two different questions.

**No new plan variant, no new command, no schema change.**
`finish_revoked_*_process` already exists and already CASes on `revoked`; the
verdict table already handles a finished lease. This task supplies the evidence
those two were missing, and nothing else.

**The failure text stops instructing humans.** `ops/task.rs:2405`'s "manual
cleanup is required" survives only for the `Present`/`Unprovable` case, and says
what is actually true: the group could not be proven gone, and the lease will
release itself when it can.

## Scope

- In scope: the presence probe; probe-only release; wiring at the reap failure
  path, the Task and Project reservation boundaries, and the stranded-recovery
  pass; the misleading CAS message; the `status_reason` prose on the paths this
  touches.
- Out of scope: a boot-time sweeper for `revoked` leases (#1000's neighbourhood);
  recovery dispatch policy (W2-267 owns it); `strand_verdict`'s classification;
  any mutation of the live registry, including W2-230's row.

## Done when

- `cargo test -p loopflow --lib ops::child ops::task engine::process` is green,
  including new tests that:
  - spawn a real process group, kill it, and prove a `revoked` lease over it
    releases to `finished` and that `reserve_task_process` then succeeds —
    the reservation path is executable **only after** the release (the same test
    asserts the reservation is refused before it);
  - hold a real live process group and prove the lease stays `revoked` and the
    reservation is refused;
  - prove `EPERM` does not release: probe a group we may not signal (pid 1's
    group, owned by root) and assert `Present`, not `Gone`;
  - **fail-closed, signal identity:** a recorded group id that does not fit
    `i32`, and one that converts to `0`, each probe `Unprovable` and do not
    release. This is the case `process_target_exists` answers `false` to; the
    test is written to go red against that function, so reusing it later cannot
    pass silently;
  - **fail-closed, tmux:** a tmux probe that cannot be spawned, and one that
    exits non-zero with unrecognized stderr, each probe `Unprovable` and do not
    release; a positively recognized no-server stderr probes `Absent`; a live
    recorded session probes `Present`. Per the wave's 2026-07-16 finding, the
    outage is modelled by *replacing* the fake `tmux` with a failing script —
    never by deleting it, since `AmbientGuard` prepends its temp bin dir to the
    real `PATH` and a deleted fake falls through to the host's real tmux;
  - prove the refusal names the lease, not the status.
- Each test is sabotage-checked: reverting the release makes it red. A test that
  passes with the fix removed is pinning the fixture.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` pass.
- One serial PR published for review.

## Measure

Registry lease states before: `active=41, finished=75, revoked=1`. After a
release ships and W2-230's Session is next asked to reserve, `revoked=0` with no
human write. The Developer Efficiency KR this serves is measured in Sessions
sitting in `failed` awaiting a manual resume; this removes one whole class of
them.
