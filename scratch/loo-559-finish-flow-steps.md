# LOO-559: settle exited agent groups under load

Status: implemented 2026-10-21; awaiting gate. No product decision is open; the
assumptions below are reversible and recorded in `scratch/questions.md`.

## Problem

On 2026-10-21, at load 60–120, two `autonomous` Flows lost finished steps:
LOO-531 at compress (`AgentProcess group 37788 death is unresolved`) and
LOO-557 at implement (group 91802). The agent had answered; closing it failed.

`close_agent_process` (`rust/loopflow/src/harness/agent_process.rs`) finds the
recorded leader live, then `terminate_process_group`
(`rust/loopflow/src/os_process.rs`) sends SIGTERM, waits 2s, sends SIGKILL,
waits 2s, and returns `false` if `OsProcess::group_is_alive` still sees any
non-zombie member. The error names no member, so the incidents left no
evidence of what survived.

## Risk findings

- **Zombies are already dead.** `group_is_alive` skips `Z`, and
  `zombie_is_dead_before_its_parent_reaps_it` proves it on macOS. The Task's
  zombie hypothesis is refuted for zombies proper.
- **Exiting processes are not.** The macOS reader (`os_process/macos.c`)
  reports only `p_stat`; anything other than `SZOMB` becomes `"S"`. A killed
  process stays `SRUN` while XNU tears it down (`P_LEXIT`, exported as
  `P_WEXIT` 0x2000 in `kp_proc.p_flag`, confirmed in the SDK's `sys/proc.h`) and becomes `SZOMB` only at the end.
  Tearing down a large Node address space under memory pressure takes seconds.
  This is the most likely cause; it fits both incidents and a 2s SIGKILL wait.
- **No final observation.** The wait loop can sample, sleep past the deadline
  and return `false` without looking again; the group may already be gone.
- **Observation is a whole-machine sample.** Each 25ms poll enumerates every
  process and reads each one's arguments (a fresh 1MB buffer per process). A C
  probe of the same syscalls: 1448 processes, ~29ms at load 18; one
  `KERN_PROC_PGRP` query: 0.03ms. The cost grows with load and inventory, and
  it is pure waste for a death judgment.
- **No logs** mention 37788 or 91802; the original survivors are unknowable.
  The fix must name them next time.

## Chosen approach

1. **Exiting counts as exited.** The macOS shim also returns `p_flag`; Rust maps
   `SZOMB` → `"Z"`, `P_WEXIT` → `"X"` (ps's "dead"), else `"S"`. One method,
   `OsProcess::has_exited()` (`Z` or `X`), replaces the two `starts_with('Z')`
   checks in `evidence` and the group judgment. A process past `exit()` can
   never run user code again, which is the same argument the zombie rule makes.
2. **Read the group, not the machine.** `OsProcess::group(pgid) ->
   io::Result<Vec<OsProcess>>` returns the members that have not exited.
   macOS: one `KERN_PROC_PGRP` sysctl in the shim (pid, state, flags, `p_comm`);
   Linux keeps `sample` filtered by pgid. `group_is_alive` is deleted;
   callers use `group(pgid)?.is_empty()`.
3. **Wait with a final look.** `terminate_process_group` keeps SIGTERM 2s, waits
   up to 10s after SIGKILL, and always observes once more after the deadline.
   It returns `anyhow::Result<()>` instead of `bool`: refusal (own or invalid
   group), observation failure and survivors each carry their own message.
4. **Name survivors.** Both failures — `death is unresolved` and `survives its
   recorded leader` (`confirmed_group_death`) — append
   `pid 1234 (sleep, S)`, using `p_comm` (never arguments, which can carry
   prompt text).

## Alternatives considered

- *Only lengthen the grace.* Keeps counting exiting processes alive and keeps
  the slow sample; it fails the next time teardown outlasts the number.
- *Settle on leader death.* `exited_leader_does_not_settle_a_surviving_group`
  is a deliberate PID-reuse and helper-cleanup guarantee; out of scope.
- *Linux `PF_EXITING` from `/proc/<pid>/stat`.* No Linux incident; deferred.

## Current → target

| | Before | After |
|---|---|---|
| macOS state | `p_stat` only | `p_stat` + `P_WEXIT` → `Z`/`X`/`S` |
| Exited test | `starts_with('Z')` in two places | `OsProcess::has_exited()` |
| Group read | `group_is_alive` over a full sample | `OsProcess::group(pgid)` |
| `terminate_process_group` | `bool`, 2s+2s, no final look | `Result`, 2s+10s, final look |
| Error | group id only | group id + survivors |

## Delete — do not maintain

- `OsProcess::group_is_alive` and every call (os_process.rs, agent_process.rs,
  human_session.rs, harness tests in claude.rs/opencode.rs/agent_process.rs).
- The `bool` return of `terminate_process_group`; callers at
  agent_process.rs (close, reap at ~608/616, test `Group` drops),
  os_process.rs and lifeline.rs tests move to the `Result`.

## Forbidden outcomes

- A second liveness predicate beside `has_exited` or a second group reader.
- Treating a failed observation as death (an `io::Error` stays unresolved).
- Logging process arguments or environment in the survivor list.

## Exclusions

- Leader-dead groups with live helpers still fail without signaling.
- LOO-557's separate retry planning error.
- Flow retry policy; a truly surviving group still fails its step.

## Demo

Under synthetic load (`for i in $(seq 32); do yes >/dev/null & done`),
`cargo test -p loopflow agent_process` passes, including a group that keeps a
lingering zombie and a TERM-ignoring helper. Forcing the leader-dead failure
prints `AgentProcess group N survives its recorded leader: pid M (sleep, S)`.

## Internal slices

One slice; this slice is the whole change.

Focused tests (Rust, `agent_process.rs` and `journal/os_process.rs`):
- `close_settles_a_group_with_a_lingering_zombie`: the test process spawns a
  second member into the leader's group (`process_group(leader)`) and never
  waits it; close returns `Ok` while the zombie remains.
- `close_waits_for_helpers_after_the_leader_exits` (existing) covers a member
  that outlives SIGTERM and dies only on SIGKILL.
- `exited_leader_does_not_settle_a_surviving_group` additionally asserts the
  error names the `sleep` helper's PID.
- `kernel_state` mapping unit test: `SZOMB` → `Z`, `SRUN|P_WEXIT` → `X`,
  `SSLEEP` → `S`.
- `group` returns the leader and helper of a live fixture and nothing after kill.

## Done when

- A step whose agent answered and was killed settles while its members are
  still tearing down or linger as zombies.
- A group that survives SIGKILL fails with its surviving PIDs and names.
- Gate: `cargo test -p loopflow --lib -- os_process agent_process` passes
  (macOS and Linux CI); `cargo clippy --all-targets -- -D warnings` clean.

Implementation note: the macOS `p_comm` copy kept stale bytes after its NUL
(`trim_end_matches` strips only trailing NULs); names now end at the first NUL.

Compress: `ProcessInfo::into_process` is the one macOS constructor;
`confirmed_group_death` reuses its last observation; `lf top` shows `X` as
stalled, like `Z`.

Check results: `cargo test -p loopflow --lib -- os_process agent_process top::`
55 passed; clippy clean. Linux → CI.

Concept review (2026-10-21): concepts hold — one exit predicate (`has_exited`), one group reader, survivors named in both failures. No change, no open choice.
