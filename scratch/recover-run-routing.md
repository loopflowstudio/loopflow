# Recover a Run across accounts and providers

## Problem

A provider failure currently breaks the identity of the work it interrupted.
PR #1080 made one-shot agent launches more resilient, but it retries inside
`launch_agent`: attempts are capture legs, account selection is re-run below the
executor, and the durable Run cannot explain or fence the handoff. PRD-38's
working branch moved failover upward, but its compatibility runner still makes
the decision from a failure string plus one `turn_had_durable_side_effect`
boolean.

PRD-20's durable incident record shows the product failure. Three immediate
Task generations retried the same unavailable Claude pool. A human handoff to
Codex finally made progress. Later, the Task failed again because one Claude
credential was missing while another account was cooling. Credential usability
and observed capacity were both available as evidence, but the executor reduced
them to “no eligible Claude account” and stopped before trying a runnable
provider route.

Recovery must preserve the thing the user is conducting: the Work, Epoch, Run,
Basis, workspace, Task/Project identity, and authored history. Only the Launch
changes. Every attempted provider process is another sequential Launch under
the same Run, with an exact account route and a fresh lease that makes the old
Launch incapable of writing.

This advances the product objective's `recover` path and the Loopflow API KR
that every Task either lands unattended or stops with an actionable
non-convergence record. It also removes the current parallel model in which
Run owns execution but `launch_agent` privately owns recovery.

## The demo

Run the deterministic resident-Task fixture. It injects subscription exhaustion
for `claude/work`, a retryable capacity failure for `claude/personal`, and then
success for the configured Codex fallback:

```bash
cargo test -p loopflow --test run_route_recovery \
  resident_task_exhausts_accounts_then_provider -- --nocapture
```

The fixture prints one Run id and three ordered Launch routes. All three rows
have the same Work, Epoch, Basis, and cwd; the successful Codex Launch advances
the original Task. A write using either prior Launch lease is rejected. No
Session, successor Work, replacement Task, or second Run exists.

## Approach

### Depend on PRD-38 at the authority seam

PRD-38 owns the sole Run executor, Session deletion, typed Launch failure, and
the primitive for non-overlapping sequential Launches. PRD-39 will not add a
second runner or copy that transition onto current main.

The integration requires these semantics from PRD-38, whatever its final names:

1. A `LaunchFailure` identifies the failed Launch, classifies retryability, and
   carries side-effect certainty as `None`, `Known`, or `Unknown`.
2. Containment must be positively `Absent` before replacement. `Present` or
   `Unprovable` leaves the Run fenced and cannot enter route policy.
3. Ending the failed Launch and rotating the Run lease is one transaction. The
   old lease is invalid before a successor Launch can start.
4. `LaunchStarting` rejects a second non-ended Launch for the Run.
5. A failed Launch remains in durable history with its exact route and failure.

PRD-38's current branch proves most of this shape, including sequential
Launches and stale-lease rejection. Its separate `LaunchEnded` then
`rotate_run_lease` calls are not yet the atomic handoff required here; PRD-39
must consume the corrected primitive rather than recreating it.

### Separate route evidence from the routing decision

Add internal, non-wire policy types under `provider_account`. They are computed
at a recovery boundary and never store credential material:

```rust
struct RouteCandidate {
    agent: AgentRoute,
    account_id: Option<ProviderAccountId>,
    admission: RouteAdmission,
    credential: CredentialUsability,
    capacity: CapacityObservation,
}

enum RouteAdmission {
    Allowed { preferred: bool },
    Denied { reason: String },
}

enum CredentialUsability {
    Usable,
    Unavailable { reason: String },
}

enum CapacityObservation {
    Unknown,
    Available,
    Strained { used_percent: u8, resets_at: Option<i64> },
    Exhausted { reason: String, resets_at: Option<i64> },
}
```

`AgentRoute` is the canonical provider/model projection of the existing agent
selection. `RouteCandidate::is_runnable()` is a derived answer, never another
stored state: admission is allowed, the credential is usable, and capacity is
not exhausted. Unknown capacity remains runnable. Strain changes ordering but
does not pretend the account is exhausted.

The three evidence axes stay independent:

- **Admission** applies account policy: automatic, explicitly selected,
  explicit-only, disabled, and the fixed outer invocation grant.
- **Credential usability** comes from the credential authority that will
  actually launch the process. A database `connected` hint is insufficient if
  the account home or forwarded token cannot be resolved.
- **Capacity** comes from provider limit windows, cooldown, and the typed
  failure just observed. A missing credential never becomes cooldown, and a
  capacity reset never makes a credential usable.

The account lease broker gains a redacted readiness description and an exact
resolution operation. Policy can inspect the accounts inside the fixed grant,
then resolve only the chosen `(provider, account_id)`. The returned
`PreparedRoute` is ephemeral and debug-redacted: it holds the exact
`LaunchRoute` plus the credential handle needed to spawn. Durable state stores
only provider, model, and account id. No secret crosses into SQLite, output, or
chat.

Accountless local providers use the existing provider-auth status as credential
evidence. A remote or leased invocation cannot fall back to ambient credentials
outside its fixed grant. Automatic recovery never broadens authority.

### Make recovery order explicit and bounded

Resolve the primary agent from the same existing precedence as today: explicit
CLI selection, skill/config selection, then the normal default. Fallbacks do
not participate in that choice. `fallback_agents` in Wave frontmatter is an
ordered recovery-only list; each entry canonicalizes through the existing
agent parser. Empty entries and duplicate fallback providers are rejected when
config is read; a fallback that resolves to the primary provider is rejected
when the Run's recovery plan is assembled.

Given a retryable, fenced Launch failure, one pure function chooses the next
move:

```rust
fn plan_route_recovery(
    failure: &LaunchFailure,
    primary: &AgentRoute,
    fallbacks: &[AgentRoute],
    readiness: &[RouteCandidate],
    launch_history: &[Launch],
) -> RecoveryDecision;
```

The order is fixed:

1. Exclude every exact route already attempted in this Run.
2. For the failed provider and model, choose the next runnable managed account.
   Explicit `--account` preferences lead once; declared account route order is
   stable, with currently strained non-preferred accounts demoted behind
   unstrained ones.
3. After that provider has no unattempted runnable account, visit
   `fallback_agents` in order. For each provider, apply the same account rules.
4. Never cycle back to a prior route. A recovery episode is bounded by the
   finite candidate set.

`--account` remains preference plus the normal account route. `--only-account`
remains a hard grant: accounts and providers outside it are unavailable even if
the Home has ambient credentials. An explicit-only account is runnable only
when the invocation explicitly selected it.

Provider continuation is route-bound. The existing
`provider_session_accounts` table proves that a provider resume token belongs
to one account. Changing account or provider clears the resume token; the next
Launch reconstructs from durable Basis, Launch/Turn receipts, and the current
workspace. Provider conversation continuity is useful when the route stays
fixed, but it is not Work continuity and may not override account exhaustion.

### Recover at one shared Run boundary

Run-bound bodies do not use `launch_agent`'s internal retry/failover loop. The
harness executes one Launch and returns PRD-38's typed failure to the shared Run
executor. Non-Run one-shot commands may retain PR #1080's bounded retry behavior;
they are outside this Task and must never nest beneath Run recovery.

The shared executor handles a retryable failure in this order:

1. Finish or stop the provider harness and prove containment `Absent`.
2. Atomically record the typed failure, end the Launch, and rotate the Run
   lease. From this commit onward the failed Launch cannot write.
3. Reconstruct route readiness from the fixed account authority and current
   capacity observations. Read the Run's Launch history for attempted routes.
4. Choose one candidate with `plan_route_recovery`.
5. Resolve that candidate exactly, record a new Launch under the rotated lease,
   and spawn it in the same cwd with the same Work, Epoch, Run, and current
   Basis. Use a continuation seed describing the prior failure and known
   effects; never replay the original provider request or tool call.

Known side effects do not block continuation because the successor is seeded
from current durable/workspace state rather than replaying the failed action.
Unknown side effects do block it. A truncated stream, incomplete tool result,
opaque handback, or any failure for which absence of effects cannot be proved
routes to `WaitOn::Effect` and User attention without consulting fallback
routes.

When no route remains, record `WaitOn::Capability` with redacted per-candidate
reasons (`credential unavailable`, `cooling until …`, `capacity exhausted`,
`outside invocation grant`). This is when missing credentials become User
attention—not when the first missing account is encountered. Work attention is
derived from the typed Wait and Launch history; do not create a retry queue,
Failure aggregate, or Feedback row.

If containment is `Present` or `Unprovable`, the Run stays stopping under its
current fence. It presents User attention from the typed Launch failure and
containment evidence, but it does not end the Launch, rotate authority, or
attempt another route.

### Implement in dependency-safe order

1. On current main, add the pure readiness projection, recovery policy, config
   validation, and exhaustive table tests. Do not wire it into Session runners.
2. After PRD-38 lands, rebase through `lf rebase` and replace its
   `classify_disconnect_recovery` / `fallback_agents` prototype at the shared
   Run boundary. Delete Run-bound use of the nested `launch_agent` retry loop.
3. Add exact route preparation to local and leased account authority, populate
   `LaunchRoute.account_id` before the provider process starts, and feed typed
   capacity/credential observations back into readiness.
4. Add resident Project and Task behavioral tests over the real store and
   shared executor. Update troubleshooting/config docs and any DTO fixtures
   changed by PRD-38's failure wire in the same pass.

## De-risking

| Question | Finding | Impact on design |
| --- | --- | --- |
| What did PRD-20 actually prove? | Its event ledger records three immediate failures on the same unavailable Claude pool, then a manual Codex handoff, followed later by a mixed missing-credential/cooling failure. | Route exhaustion must be machine-evaluable and cross provider automatically; retrying the same provider generation is the bug. |
| Does same-provider account failover already work anywhere? | PR #1080 records a hard limit, then `select_provider_account` can choose the next account. It happens inside `launch_agent`, below durable Run authority. | Reuse health recording and account ordering; move the decision to the Run boundary and make each process attempt a Launch. |
| Are credentials and capacity already distinct in storage? | Mostly: `credential_state`, routing state, cooldown, and limit-window rows are separate. Selection collapses them into `Option<ProviderAccountSelection>` and error prose; forwarded lease setup silently skips unusable fallback credentials. | Keep the storage facts; replace the lossy selection result with a normalized readiness projection and redacted reasons. |
| Can unknown capacity be treated as unavailable? | No. New or unobserved accounts have no limit row. Treating absence as exhaustion would make every fresh route unrunnable. | `CapacityObservation::Unknown` is runnable; only a positive exhaustion/cooldown observation blocks. |
| Can a provider session resume on another account? | Loopflow deliberately pins `(provider, provider_session_id)` to one account and prioritizes that account on resume. | Clear resume tokens whenever account or provider changes. Durable Basis/workspace, not vendor conversation, carries recovery. |
| Does PRD-38 already prevent overlapping Launches? | Its branch rejects a new Launch while one is live and rotates the Run lease after a Launch ends. The end and rotation are currently separate transactions. | Require PRD-38's final primitive to make end-and-rotate atomic; otherwise a crash or stale body can act in the seam. |
| Is a boolean side-effect flag sufficient? | No. “No completed Command/File observed” is not proof of no side effect after a stream truncation. PRD-38's prototype even permits provider failover when the boolean is true. | Consume a three-way PRD-38 effect certainty. `Unknown` always fences; `Known` continues from state without replay. |
| Can fallback use any local credential it finds? | No. `AccountSelection` and the broker intentionally freeze descendant authority. `--only-account` exposes exactly its selected accounts, and remote Homes may have no ambient credential. | Build candidates only from the fixed invocation authority. Recovery never escalates or widens credential access. |
| Where should no-route attention live? | `WaitOn::Capability` and `WaitOn::Effect` already express the two waits. A new Failure/Decision table would duplicate Run truth. | Derive User attention from the Wait plus typed Launch failure/history; add no lifecycle noun. |
| What happens to PR #1080's retry loop? | Leaving it active under Run recovery creates nested attempts that are invisible as Launches and can consume several accounts before policy sees one failure. | Disable internal retry for Run-bound bodies. Preserve it only for explicitly out-of-scope non-Run one-shots. |
| Does fallback become normal agent selection? | PRD-38 currently reads fallback config only after failure, which is the right boundary. | Keep primary resolution unchanged. Validate and read `fallback_agents` only for recovery after a typed retryable failure. |

## Alternatives considered

| Approach | Tradeoff | Why not |
| --- | --- | --- |
| Keep retries inside `launch_agent` and mirror them into Launch rows | Smallest diff; preserves #1080 directly | The lower layer does not own Work/Basis, containment, or Run lease rotation. Mirroring recreates PRD-38's Session/Run split one level lower. |
| Create a new Run for each account/provider attempt | Existing Run reservation already models recovery | Breaks the user's continuous record, changes Basis/authority, and makes one provider incident look like several pieces of work. |
| Choose a fallback provider before the primary Launch whenever it looks healthier | Avoids an expected failure | Makes recovery a hidden scheduler and overrides explicit CLI/config/skill intent. The selected agent must remain primary. |
| Store one `available` boolean on each account | Easy query and UI | It necessarily conflates missing credentials, policy exclusion, stale/unknown capacity, strain, and hard exhaustion—the exact incident cause. |
| Permit failover whenever no completed side effect was observed | More recoveries | A truncated provider stream can hide an effect. Absence of evidence is not the fence PRD-38 promises. |
| Retry the exhausted account with its provider resume token | Preserves conversation | The capacity condition remains and the resume token pins selection to that account. It produces the PRD-20 retry loop. |
| Let a parent Project resolve missing credentials for its Task | Keeps the human out longer | Credentials and ambiguous external effects require User authority. A parent Run cannot authenticate or attest an unknown side effect on the user's behalf. |
| Persist a recovery queue or route cursor | Simple restart bookkeeping | Launch history already is the attempted-route ledger, config/grant is the candidate source, and the policy is deterministic. Another queue can drift. |

## Key decisions

**A route is evidence, not merely an agent string.** Exact provider, model, and
account identity are recorded on Launch. Admission, credentials, and capacity
remain separate internal evidence so diagnostics can say what the user can
repair.

**The primary agent never changes until a failure.** Account health may choose
which account serves that agent, but cross-provider fallback is invoked only by
a typed retryable Launch failure.

**`fallback_agents` replaces `backup_agent`, with no alias.** The ordered list
describes recovery policy directly; retaining both spellings would create two
sources of route order inside Wave config.

**Recovery is finite.** The same exact route is attempted at most once per Run.
The system cannot ping-pong between providers or wake forever on a cooling
account.

**Known effects continue; unknown effects stop.** A successor can read known
receipts and workspace changes. It cannot safely infer what an interrupted,
opaque, or truncated provider may have changed.

**Lease rotation precedes policy execution.** Even an error while computing or
preparing the next route leaves the prior writer fenced. Operationally boring
beats a clever handoff seam at 2 a.m.

**History, not a queue, is the cursor.** The next candidate is derived from
ordered config/grant minus durable Launch routes. Restarting the executor
reaches the same bounded answer without replay bookkeeping.

Wild success is visually plain: one Run timeline tells the whole story—Claude
account A exhausted, Claude account B unavailable, Codex account C completed—
and the Task keeps moving without a human. Wild failure is two retry engines
competing, repeatedly selecting an account whose credential/capacity reason was
flattened into a string, while a stale process still owns a write token. The
design removes all three conditions rather than adding retry count.

## Scope

- In scope: normalized internal route-readiness types; exact local and leased
  account preparation; ordered recovery-only `fallback_agents`; pure bounded
  policy; Run-bound removal of nested retries; same-Run sequential Launch
  integration after PRD-38; redacted exhaustion evidence; resident Project and
  Task behavioral tests; affected config/troubleshooting docs and fixtures.
- In scope: Claude and Codex managed accounts, plus accountless providers only
  when the current Home/invocation authority can positively establish usable
  credentials.
- Out of scope: implementing or copying PRD-38's executor, Session deletion,
  typed failure wire, containment probe, or atomic Launch replacement.
- Out of scope: changing normal CLI/config/skill agent precedence, dynamic
  cost/quality routing, load balancing successful Launches, provider/model
  benchmarking, credential setup UI, or widening `--only-account` authority.
- Out of scope: automatic recovery from unknown side effects, unprovable
  containment, permanent invalid-request failures, and non-Run one-shot retry
  semantics.

## Done when

Pure policy proofs cover:

- usable credential + unknown capacity is runnable;
- missing credential + available capacity is not runnable;
- usable credential + active exhaustion is not runnable;
- strain demotes but does not exclude an account;
- explicit-only and `--only-account` admission is preserved;
- an exhausted primary account chooses the next same-provider account;
- same-provider exhaustion chooses the first runnable fallback provider;
- missing credentials are skipped until every alternative is exhausted;
- every exact route is attempted at most once;
- changing account or provider clears the resume token;
- unknown side effects and unprovable containment never choose a route.

Shared executor proofs cover both resident Project and resident Task Work:

1. A retryable account failure creates a second Launch in the same Run.
2. Exhausting that provider creates the ordered fallback Launch in that Run.
3. Work id, Epoch id, Run id, Basis, cwd/worktree, Task/Project identity, PR,
   and prior Launch/Turn history remain identical across the handoff.
4. The prior lease cannot advance, complete, steer, or start another Launch
   after atomic replacement; two Launches are never live concurrently.
5. A credential failure before provider work does not become User attention
   while another runnable route exists.
6. No route produces a redacted capability Wait and User attention.
7. Unknown effects produce an effect Wait and User attention with zero
   successor Launches.
8. Restart after lease rotation but before successor spawn derives and launches
   exactly the next unattempted route.

Verification:

```bash
cargo test -p loopflow route_readiness
cargo test -p loopflow route_recovery_policy
cargo test -p loopflow --test run_route_recovery
cargo test -p loopflow --test run_authority_tests
cargo fmt --check
cargo clippy -p loopflow --all-targets -- -D warnings
```

The real Product Wave dogfood repeats the demo with one Task and one Project
Run. `lf runs --json` must show one Run each, multiple exact account/provider
Launch routes, no overlap, and either successful continuation or explicit User
attention.

## Measure

| Signal | Baseline | Target |
| --- | --- | --- |
| Human provider handoffs in the recovery fixture | 1 (PRD-20 required Claude → Codex) | 0 |
| Runs created for one recovery sequence | Not authoritative below `launch_agent` | Exactly 1 |
| Exact Launch routes attempted more than once per Run | Possible through nested retry/reselection | 0 |
| Concurrent live Launches per Run | PRD-38 invariant under construction | Maximum 1 |
| Missing-credential attention while another route is runnable | PRD-20 stopped on mixed account state | 0 |
| Automatic retries with unknown side effects | Boolean cannot prove absence | 0 |

## Wave alignment

The design serves the product objective directly: a user can recover work
without caring which account, provider process, or machine owns the machinery.
It advances two Loopflow API proofs:

- **“Task loops earn trust by streak.”** Retryable provider exhaustion becomes
  bounded unattended recovery; fenced/no-route outcomes become actionable
  non-convergence instead of silent stalls or human rescue.
- **“One model everywhere, continuously.”** CLI, Mac, iOS, prompts, and workers
  continue to inspect one Run → Launch history. There is no provider retry
  ledger beside it.

The new durable risk is route history exposing account identity. Mitigation:
persist only the existing stable account id, never login tokens or credential
material; user-facing diagnostics use redacted reasons. This belongs in Wave
memory only after implementation proves the contract.
