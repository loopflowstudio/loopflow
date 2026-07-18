# Recover a Run across accounts and the backup agent

## Problem

A provider failure currently breaks the identity of the work it interrupted. PR
#1080 made one-shot agent launches more resilient, but it retries inside
`launch_agent`: attempts are capture legs, account selection is re-run below the
executor, and the durable Run cannot explain or fence the handoff. PRD-38 moved
replacement *safety* up to a sole Run executor: it owns containment, effect
certainty, Waits, fencing, stale-writer rejection, and the atomic primitive that
ends one Launch and rotates the Run lease before a successor can start.

PRD-39 is the route chooser that sits on top of that boundary. It does not
re-specify, implement, or re-prove any of PRD-38's safety semantics. Its job is
narrow: given permission to replace, pick the next exact route; given no route,
hand back a typed capability wait.

PRD-20's durable incident record shows the product failure this is fixing. Three
immediate Task generations retried the same unavailable Claude pool. A human
handoff to Codex finally made progress. Later, the Task failed again because one
Claude credential was missing while another account was cooling. Credential
usability and observed capacity were both available as evidence, but
`account_unavailable_reason` collapses `credential_state`, `routing_state`,
`cooldown`, and limit-window rows into one prose string and stops before trying
a runnable route.

Recovery must preserve the thing the user is conducting: the Work, Epoch, Run,
Basis, workspace, Task/Project identity, and authored history. Only the Launch
changes. Every attempted provider process is another sequential Launch under the
same Run, with an exact account route and a fresh lease that makes the old
Launch incapable of writing.

This advances the product objective's `recover` path and the Loopflow API KR
that every Task either lands unattended or stops with an actionable
non-convergence record.

## The demo

Run the deterministic resident-Task fixture. It injects a retryable capacity
failure for the primary `claude/work` account, a retryable capacity failure for
the other same-provider `claude/personal` account, and then success for the
configured singular `backup_agent`:

```bash
cargo test -p loopflow --test run_route_recovery \
  same_provider_then_backup_agent -- --nocapture
```

The fixture prints one Run id and three ordered Launch routes:
`claude/work` → `claude/personal` → the `backup_agent` provider. All three rows
have the same Work, Epoch, Basis, and cwd; the successful `backup_agent` Launch
advances the original Task. A write using either prior Launch lease is rejected
— evidence supplied by PRD-38's atomic replacement primitive, not re-proved
here. No second Run, successor Work, replacement Task, or new Session exists.

## Approach

### Depend on PRD-38 at the authority seam

PRD-38 exclusively owns the sole Run executor, containment
(`Absent`/`Present`/`Unprovable`), effect certainty, Waits
(`WaitOn::Capability`/`WaitOn::Effect`), fencing and stale-writer rejection, and
the atomic end-and-rotate-lease primitive that replaces one Launch with the next.

PRD-39 consumes that boundary. At a recovery moment PRD-39 receives exactly one
of two inputs from PRD-38:

- **Permission to replace** — containment is `Absent` and effect certainty is
  `Known` or `None`. PRD-39 chooses the next route or reports no route.
- **A typed stop** — containment is `Present`/`Unprovable`, or effect certainty
  is `Unknown`. PRD-39 is not consulted; PRD-38 records the `WaitOn::Effect` and
  presents User attention.

PRD-39 must not specify, implement, re-prove, or rename those semantics. Its
policy only runs on permission to replace.

### Model one exact route, reuse every account field

An `ExactRoute` is the canonical agent (provider + model) plus an optional
existing `ProviderAccountId`. It is stored on the existing `LaunchRoute`, which
already carries `provider`, `model`, and `account_id: Option<String>`. No new
durable field, no new lifecycle noun.

The only addition is a typed projection of *why a route is unavailable*,
computed at a recovery boundary from existing `ProviderAccount` fields and the
fixed invocation grant, and never persisted as new state:

```rust
struct ExactRoute {
    agent: AgentRoute,
    account_id: Option<ProviderAccountId>,
}

enum RouteUnavailable {
    Credential,                          // credential_state != Connected,
                                         //   or forwarded-lease token cannot resolve
    Capacity { resets_at: Option<i64> }, // cooldown_until > now, or an
                                         //   AccountLimitWindow is positively exhausted
    Policy,                              // effective_routing_state is ExplicitOnly or
                                         //   Disabled, or the route is outside the
                                         //   fixed --only-account / explicit-only grant
}

struct RouteCandidate {
    route: ExactRoute,
    readiness: Result<(), RouteUnavailable>,
    strained: bool, // demotes ordering, never excludes
}
```

The three unavailability axes map directly onto current account fields, with no
overlap:

- **Credential** ← `ProviderAccount.credential_state` (`Connected`/`Missing`)
  and the forwarded-lease resolution that actually spawns the process. A missing
  credential never becomes cooldown.
- **Capacity** ← `cooldown_until`/`cooldown_reason` and `AccountLimitWindow`
  rows. A capacity reset never makes a credential usable.
- **Policy** ← `effective_routing_state` (`Automatic`/`ExplicitOnly`/`Disabled`)
  and the fixed outer grant (`--only-account`, explicit-only selection).

`RouteCandidate` is runnable when `readiness` is `Ok(())`. Unknown capacity
(no limit row, no cooldown) stays runnable; strain demotes ordering but does
not exclude. The projection is debug-redacted; no secret crosses into SQLite,
output, or chat. Accountless local providers use the existing provider-auth
status as credential evidence. Automatic recovery never broadens authority
beyond the fixed grant.

### Make recovery order bounded and simple

Primary agent resolution is unchanged: explicit CLI selection, skill/config
selection, then the normal default. `backup_agent` is not consulted for primary
selection.

One pure function chooses the next route after a retryable failure, on
permission to replace:

```rust
fn plan_route_recovery(
    ordered_candidates: &[RouteCandidate], // primary provider accounts first,
                                            //   then backup_agent's accounts
    chain_excluded: &[ExactRoute],         // current consecutive chain only
) -> RecoveryChoice;

enum RecoveryChoice {
    Launch(ExactRoute),
    AwaitCapability { reasons: Vec<(ExactRoute, RouteUnavailable)> },
}
```

The caller builds `ordered_candidates` by running the *same* existing account
chooser for the primary agent's provider, then — if `backup_agent` is configured
— for the backup agent's provider. The policy is the bounded choice over that
list:

1. Skip any candidate whose `ExactRoute` is in `chain_excluded` (current
   consecutive chain only — see below).
2. Skip any candidate whose `readiness` is `Err(_)`.
3. Among remaining same-provider candidates, prefer explicit `--account`
   selections, then declared account-route order, demoting strained
   non-preferred accounts behind unstrained ones.
4. Take the first runnable same-provider candidate. If none, take the first
   runnable `backup_agent` candidate.
5. If nothing remains, return `AwaitCapability` with the typed per-route
   reasons.

`--account` stays preference plus the normal account route. `--only-account`
stays a hard grant: accounts and providers outside it are unavailable even if
the Home has ambient credentials. An explicit-only account is runnable only when
the invocation explicitly selected it. Authority never broadens beyond the
existing account grant.

Provider continuation is route-bound. The existing
`provider_session_accounts` table pins a provider resume token to one account.
Changing account or provider clears the resume token; the next Launch
reconstructs from durable Basis, Launch/Turn receipts, and the current
workspace. Vendor conversation continuity is useful when the route stays fixed,
but it is not Work continuity and does not override account exhaustion.

### Exclude routes only within the current consecutive recovery chain

A **recovery chain** is a maximal sequence of Launches in one Run where each
Launch after the first is a recovery replacement of its immediately prior
Launch. The chain breaks when:

- a Launch in the chain succeeds and advances the Run — the next Launch is fresh
  work, not a replacement; or
- the Run enters a Wait or stops — the next Launch, when it comes, is not a
  direct replacement of the last failed one.

`chain_excluded` is the set of `ExactRoute`s of Launches in the *current* chain
only. A long-lived Run may attempt the same exact route again in a later chain,
because the condition that made it unavailable may have cleared (a cooldown
expired, a credential was restored, a limit window reset). Launch history for
the whole Run is **not** a permanent blacklist.

Restart derives the same current-chain exclusions deterministically: walk back
from the failed Launch through its recovery-predecessor links (same Run, same
Work/Epoch, each terminated by a retryable failure and replaced by the next),
stopping at the first Launch that succeeded or has no such link. That walk reads
only durable Launch rows, so a restarted executor reaches the same bounded
answer without replay bookkeeping or a persisted cursor.

### Recover at the shared Run boundary

Run-bound bodies do not use `launch_agent`'s internal retry/failover loop. The
harness executes one Launch and returns PRD-38's typed failure to the shared Run
executor. Non-Run one-shot commands may retain PR #1080's bounded retry
behavior; they are outside this Task and must never nest beneath Run recovery.

On a retryable failure with permission to replace, PRD-39 does exactly:

1. Read the current-chain history from durable Launch rows.
2. Project `RouteCandidate`s from the fixed account grant and current
   `ProviderAccount` fields, primary provider first then `backup_agent`.
3. Call `plan_route_recovery`.
4. Hand the chosen `ExactRoute` back to PRD-38, which resolves it, records a new
   Launch under the rotated lease, and spawns it in the same cwd with the same
   Work, Epoch, Run, and current Basis. The continuation seed describes the prior
   failure and known effects; it never replays the original provider request or
   tool call.

When `plan_route_recovery` returns `AwaitCapability`, PRD-39 hands the typed
per-route reasons to PRD-38, which records `WaitOn::Capability` and presents
User attention. This is when missing credentials become User attention — not
when the first missing account is encountered. Work attention is derived from
the typed Wait and Launch history; PRD-39 adds no retry queue, Failure
aggregate, or Feedback row.

### Implement in dependency-safe order

1. On current main, add the pure `ExactRoute` projection, `plan_route_recovery`,
   chain-derivation helper, and exhaustive table tests. Do not wire it into
   Session runners.
2. After PRD-38 lands, rebase through `lf rebase` and consume its
   permission-to-replace / typed-stop boundary. Delete Run-bound use of the
   nested `launch_agent` retry loop and `classify_disconnect_recovery` for
   Run-bound bodies.
3. Populate `LaunchRoute.account_id` before the provider process starts, and
   feed typed capacity/credential observations from each failed Launch back into
   the next projection.
4. Add resident Project and Task behavioral tests over the real store and shared
   executor. Update troubleshooting/config docs and any DTO fixtures changed by
   PRD-38's failure wire in the same pass.

## De-risking

| Question | Finding | Impact on design |
| --- | --- | --- |
| What did PRD-20 actually prove? | Its event ledger records three repeated failures on the same unavailable Claude pool, then a manual Codex handoff, followed later by a mixed missing-credential/cooling failure. | Route exhaustion must be machine-evaluable; the typed unavailable reason must distinguish credential, capacity, and policy so a runnable route is not hidden behind one string. |
| Are credentials and capacity already distinct in storage? | Yes: `credential_state`, `routing_state`, cooldown, and limit-window rows are separate. `account_unavailable_reason` collapses them into prose. | Keep the storage facts; project them into a typed `RouteUnavailable` at the boundary instead of flattening them. |
| Can unknown capacity be treated as unavailable? | No. New or unobserved accounts have no limit row. Treating absence as exhaustion would make every fresh route unrunnable. | Unknown capacity stays runnable; only a positive cooldown/exhaustion observation blocks. |
| Can a provider session resume on another account? | Loopflow deliberately pins `(provider, provider_session_id)` to one account. | Clear resume tokens whenever account or provider changes. Durable Basis/workspace, not vendor conversation, carries recovery. |
| Does `backup_agent` become normal agent selection? | No — it is read only after a typed retryable failure exhausts same-provider accounts. | Keep primary resolution unchanged; consult `backup_agent` only at the recovery boundary. |
| Should route exclusion cover the whole Run? | No — a long-lived Run may legitimately reuse a route in a later chain once its unavailability clears. | Scope exclusion to the current consecutive recovery chain; derive it from Launch history on restart. |
| Where should no-route attention live? | `WaitOn::Capability` already expresses the wait and is owned by PRD-38. | PRD-39 returns typed reasons; PRD-38 records the Wait. No new lifecycle noun. |

## Alternatives considered

| Approach | Tradeoff | Why not |
| --- | --- | --- |
| Keep retries inside `launch_agent` and mirror them into Launch rows | Smallest diff; preserves #1080 directly | The lower layer does not own Work/Basis, containment, or Run lease rotation. Mirroring recreates PRD-38's Run split one level lower. |
| Create a new Run for each account/provider attempt | Existing Run reservation already models recovery | Breaks the user's continuous record, changes Basis/authority, and makes one provider incident look like several pieces of work. |
| Promote `backup_agent` to an ordered recovery list | More recovery options per wave | The wave already has a singular `backup_agent`. A second spelling creates two sources of route order in config and a migration with no caller benefit. Keep the singular field. |
| Store one `available` boolean on each account | Easy query and UI | Conflates missing credentials, policy exclusion, stale/unknown capacity, strain, and hard exhaustion — the exact incident cause. |
| Persist a recovery queue or route cursor | Simple restart bookkeeping | Launch history is already the attempted-route ledger, config/grant is the candidate source, and chain exclusion is derivable. Another cursor can drift. |
| Make Run-history exclusion permanent | Strongest guarantee against re-trying a bad route | A cooling account or missing credential may clear later in the same long-lived Run. Permanent exclusion would turn a transient incident into a permanent stop. |

## Key decisions

**A route is evidence, not merely an agent string.** Exact provider, model, and
account id are recorded on `LaunchRoute`. The typed unavailable reason
(Credential/Policy/Capacity) is a projection of existing `ProviderAccount`
fields and the fixed grant — computed at the boundary, never persisted as new
state.

**The primary agent never changes until a failure.** Account health may choose
which account serves that agent, but the singular `backup_agent` is invoked only
after same-provider accounts are exhausted.

**`backup_agent` stays singular.** No ordered list, no alias, no migration, and
no config validation for a list. One configured backup, consulted once, through
the same account chooser as the primary provider.

**Recovery exclusion is chain-scoped, not Run-scoped.** A later chain in a
long-lived Run may reuse a route whose earlier unavailability has cleared. The
chain boundary is derivable from durable Launch history, so restart reaches the
same bounded answer.

**History, not a queue, is the cursor.** The next candidate is derived from
ordered config/grant minus current-chain durable Launch routes. No persisted
cursor, no replay bookkeeping.

**PRD-39 owns route choice; PRD-38 owns everything else.** Containment, effect
certainty, Waits, fencing, stale-writer rejection, and atomic Launch
replacement are PRD-38's contract. PRD-39 receives permission to replace or a
typed stop; it never re-specifies or re-proves those semantics.

Wild success is visually plain: one Run timeline tells the whole story —
Claude `work` exhausted, Claude `personal` cooling, `backup_agent` Codex
completed — and the Task keeps moving without a human. Wild failure is two
retry engines competing while a stale process still owns a write token. The
design removes the second engine and the string-flattening, not by adding retry
count.

## Scope

- In scope: typed `ExactRoute` unavailability projection over existing account
  fields; pure bounded `plan_route_recovery`; same-provider-then-`backup_agent`
  ordering; chain-scoped route exclusion; Run-bound removal of nested retries;
  same-Run sequential Launch integration after PRD-38; redacted typed exhaustion
  evidence; resident Project and Task behavioral tests; affected
  config/troubleshooting docs and DTO fixtures changed by PRD-38's failure wire.
- In scope: Claude and Codex managed accounts, plus accountless providers only
  when the current Home/invocation authority can positively establish usable
  credentials.
- Out of scope: PRD-38's executor, containment, effect certainty, Waits,
  fencing, stale-writer rejection, and atomic Launch replacement primitive.
- Out of scope: promoting `backup_agent` to an ordered list, alias, or
  migration, or adding config validation for one.
- Out of scope: changing normal CLI/config/skill agent precedence, dynamic
  cost/quality routing, load balancing successful Launches, provider/model
  benchmarking, credential setup UI, or widening `--only-account` authority.
- Out of scope: automatic recovery from unknown side effects, unprovable
  containment, permanent invalid-request failures, and non-Run one-shot retry
  semantics.

## Done when

Pure policy proofs cover:

- usable credential + unknown capacity is runnable;
- missing credential + available capacity is not runnable, typed `Credential`;
- usable credential + active cooldown/exhaustion is not runnable, typed
  `Capacity`;
- usable credential + `ExplicitOnly`/`Disabled`/outside-grant is not runnable,
  typed `Policy`;
- strain demotes but does not exclude an account;
- an exhausted primary account chooses the next same-provider account before
  `backup_agent`;
- same-provider exhaustion chooses the configured singular `backup_agent` when
  one of its accounts is runnable;
- `backup_agent` absent or unusable with no same-provider account returns
  `AwaitCapability` with typed per-route reasons;
- every exact route is attempted at most once within a consecutive chain;
- a route excluded in an earlier chain is eligible again in a later chain of the
  same Run;
- changing account or provider clears the resume token;
- restart derives the same current-chain exclusions from durable Launch history.

Shared executor proofs cover both resident Project and resident Task Work:

1. A retryable account failure creates a second Launch in the same Run with a
   different same-provider account.
2. Same-provider exhaustion creates the singular `backup_agent` Launch in that
   Run.
3. Work id, Epoch id, Run id, Basis, cwd/worktree, Task/Project identity, PR,
   and prior Launch/Turn history remain identical across the handoff.
4. The prior lease cannot advance, complete, steer, or start another Launch
   after replacement — evidence supplied by PRD-38's atomic replacement
   primitive, not re-proved here.
5. A credential failure before provider work does not become User attention
   while another usable same-provider route exists.
6. No route produces a redacted `WaitOn::Capability` and User attention (typed
   stop recorded by PRD-38 from PRD-39's `AwaitCapability`).
7. Unknown effects / unprovable containment produce a typed stop from PRD-38
   with zero successor Launches — PRD-39 is not consulted.
8. Restart after lease rotation but before successor spawn derives and launches
   exactly the next unattempted current-chain route.

Verification:

```bash
cargo test -p loopflow exact_route_projection
cargo test -p loopflow route_recovery_policy
cargo test -p loopflow --test run_route_recovery
cargo test -p loopflow --test run_authority_tests
cargo fmt --check
cargo clippy -p loopflow --all-targets -- -D warnings
```

The real Product Wave dogfood repeats the demo with one Task and one Project
Run. `lf runs --json` must show one Run each, multiple exact account/provider
Launch routes, no overlap, and either successful continuation or a typed
User-attention Wait.

## Measure

| Signal | Baseline | Target |
| --- | --- | --- |
| Human provider handoffs in the recovery fixture | 1 (PRD-20 required Claude → Codex) | 0 |
| Runs created for one recovery sequence | Not authoritative below `launch_agent` | Exactly 1 (same-Run continuity) |
| Exact Launch routes attempted more than once within a consecutive chain | Possible through nested retry/reselection | 0 |
| Routes from an earlier chain reused in a later chain of the same Run | Not permitted (Run history was a blacklist) | Permitted |
| Same-provider account attempted before `backup_agent` | Not guaranteed | Always (same-provider-before-backup order) |
| Typed unavailable reason per skipped route | Flattened into one prose string | One of `Credential`/`Capacity`/`Policy` |
| Missing-credential attention while another usable route exists | PRD-20 stopped on mixed account state | 0 |
| Concurrent live Launches per Run | PRD-38 invariant under construction | Maximum 1 (evidence supplied by PRD-38) |

## Wave alignment

The design serves the product objective directly: a user can recover work
without caring which account, provider process, or machine owns the machinery.
It advances two Loopflow API proofs:

- **"Task loops earn trust by streak."** Retryable provider exhaustion becomes
  bounded unattended recovery; no-route outcomes become actionable
  non-convergence instead of silent stalls or human rescue.
- **"One model everywhere, continuously."** CLI, Mac, iOS, prompts, and workers
  continue to inspect one Run → Launch history. There is no provider retry
  ledger beside it.

The new durable risk is route history exposing account identity. Mitigation:
persist only the existing stable account id on `LaunchRoute`, never login tokens
or credential material; user-facing diagnostics use the redacted typed reasons.
This belongs in Wave memory only after implementation proves the contract.
