# 5 Whys: Available Claude Capacity Did Not Rescue PRD-20

## The Problem

PRD-20 blocked on `provider_rate_limited: seven_day` even though the
`jackstah@gmail.com` Claude subscription still had weekly capacity.

## Chain

Blocked Task → no resident account retry → alternate credential ineligible →
capacity and execution readiness diverged → executor recovery is fragmented

**Problem**: PRD-20's `ci-fix` body stopped on the exhausted
`jack@loopflow.studio` Claude account and left PR #1052 unrepaired.

**Why 1**: The Task runner treats `provider_rate_limited` as an ordinary body
failure. With an active PR, ordinary provider failures become an infrastructure
block instead of starting another provider Launch.

↳ *Could we have caught this earlier?* A resident-run test should prove that
account exhaustion selects another account or provider without changing Work,
Run, Basis, or the worktree.

**Why 2**: Same-provider account failover exists in `launch_agent`, but resident
Task and Project bodies drive `Harness` directly. They resolve an account once
at `Harness::start` and do not share `launch_agent`'s retry loop.

↳ *What process allowed this?* Recovery was added to one executor while the
codebase still had separate one-shot, Task, and Project execution paths.

**Why 3**: The routed alternate Claude account, `primary`, was not eligible.
Its subscription polling showed 32% weekly usage, but its managed Claude home
reported `loggedIn: false` and the durable account row correctly said
`credential_state=missing`. Browser access venues can perform a future OAuth
ceremony; they are not credentials a headless Run can spend.

↳ *What assumption was wrong?* “The account has capacity” was treated as
equivalent to “the executor can authenticate as the account.” Those are
separate facts.

**Why 4**: Routing silently skips missing fallback credentials at root-lease
preparation. The account list exposes `missing`, but a blocked Run does not
surface the available login ceremony as its next action, and automatic login
cannot safely happen headlessly because Claude OAuth requires user approval.

↳ *Why was that assumption encoded?* Account selection, browser access,
subscription observation, and execution recovery are separate subsystems with
no normalized readiness result.

**Why 5 (Root)**: Loopflow has multiple executor loops with different recovery
semantics. A provider failure is therefore interpreted according to the path
that happened to launch it, rather than as one durable Run transition over
normalized account and provider availability.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
| --- | --- | --- |
| Why 3 | Why did the Claude `primary` login become unusable after its last connection attempt? | Medium |
| Why 4 | Should missing credentials create User attention before all runnable alternatives are exhausted? | High |
| Why 5 | Which failure classes are safe to resume on another provider after unknown native side effects? | High |

## Fixes

| Level | Fix | Prevents |
| --- | --- | --- |
| Immediate | Make Codex the implicit agent; keep only intentional Claude skill marks | New work consuming the exhausted Claude route accidentally |
| Immediate | Reconnect Claude `primary` through a mapped access venue | This account remaining unusable despite capacity |
| Structural | Treat account exhaustion and disconnects as new Launches in the same Run, with ordered cross-provider fallbacks | Resident work blocking on one provider |
| Systemic | Delete the separate Session executors and put one recovery policy behind Run/Launch | One-shot and resident recovery drifting again |

## Changes to Implement

- [x] Define one implicit agent: `codex`.
- [x] Remove redundant `default_agent: codex` metadata.
- [x] Preserve the explicit Claude skills and prove their override.
- [ ] Reconnect `claude/primary` with `lf auth connect claude primary`.
- [ ] Make PRD-38's shared Run executor own account and provider fallback.
- [ ] Add a durable resident exhaustion test across Work, Run, Launch, and Basis.
