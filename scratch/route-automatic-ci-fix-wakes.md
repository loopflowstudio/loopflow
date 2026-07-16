# Receive GitHub CI triggers in `lfd` and steer the owning Task

## Intent

One `lfd` per Loopflow Home receives signed GitHub CI webhooks even when every
Wave and Project supervisor is stopped. It persists the delivery, resolves the
open PR to its current Task Session, and sends that Task one durable trigger.
The trigger steers a live body or wakes a sleeping body into the existing
bounded `ci-fix` flow.

The outcome is operational, not architectural: the actual Waves on this Home
detect broken CI, respond without a human noticing first, repair the same PR,
and land it. Loopflow must make that visible across repositories: how many CI
repairs a Task needed, how long detection and response took, how long the PR
took to return to green and merge, and which incidents remain blocked.

`lfd` is the doorway, not the orchestrator. It verifies, records, routes, and
sends. Task control still owns command delivery and leases; the Task runner
still owns the meaning and settlement of `ci-fix`.

```text
GitHub check_run
      |
      v
POST /github/webhook                 lfd: receive
  verify -> provider_deliveries
      |
      v
repo + PR + head -> current Task     lfd: route
      |
      v
refresh CiObservation
      |
      v
Trigger(identity, text)              lfd: send
      |
      +-- live Task ----> steer current turn
      |
      `-- sleeping Task -> launch one ci-fix body
```

## What the history says

Three generations of the design line up around the same boundary.

1. The old GitHub hook router already had the useful edge mechanics: a bounded
   hook-only route, `X-Hub-Signature-256` verification, `check_run` parsing,
   repo/PR routing, and plan-before-deliver tests.
2. Its trigger engine was the wrong center: persisted signal/flow
   configuration, pending activations, an event hub, scheduler, executor,
   in-memory CI deduplication, and detached `lf` processes after the response.
   None of that returns.
3. W2-206 established the current ingress pattern. A provider event compiles to
   the Task control language: Linear edits become `Steer`; comments become
   `FollowUp`. Provider code does not bypass the command/directive store.
4. W2-224 established one Home-level `lfd`, signed provider routes, and the
   durable `provider_deliveries` inbox. It intentionally deferred the GitHub
   adapter and wake behavior to this Task.
5. W2-229, W2-231, and W2-232 already built the bounded `ci-fix` lifecycle,
   same-PR repair, infrastructure blocking, and one-body settlement. This Task
   should feed that lifecycle, not rebuild it.

The old `signal -> flow` idea therefore shrinks to a message sent through the
existing Task command ledger. There is no trigger registry and no activation
queue.

## Measure the current loop first

Build the operational readout before changing wake ownership. The existing
Project-poll path provides a baseline; the webhook path should improve a
measured loop rather than merely pass a new fixture.

Persist one `CiIncident` for each semantic trigger identity:

```rust
pub struct CiIncident {
    pub identity: String,
    pub task_session_id: TaskSessionId,
    pub pr_id: TaskPrId,
    pub repo: String,
    pub pr_number: u32,
    pub failed_head_sha: String,
    pub failure_set: Vec<String>,
    pub provider_completed_at: Option<OffsetDateTime>,
    pub poll_observed_at: Option<OffsetDateTime>,
    pub webhook_received_at: Option<OffsetDateTime>,
    pub trigger_command_id: Option<ChildCommandId>,
    pub responded_at: Option<OffsetDateTime>,
    pub green_at: Option<OffsetDateTime>,
    pub merged_at: Option<OffsetDateTime>,
    pub blocked_at: Option<OffsetDateTime>,
    pub blocked_reason: Option<String>,
}
```

This is historical outcome evidence, not another queue. The Task PR remains
the current CI truth and the child command remains the wake truth. The incident
joins their milestones after those current rows move on to another head.

Both observers call one idempotent `observe_ci_incident` operation:

- the current Project reconciliation records `poll_observed_at`;
- `lfd` records GitHub's `check_run.completed_at` and local
  `webhook_received_at`;
- persisting the Trigger links `trigger_command_id`;
- live delivery or ci-fix generation birth records `responded_at`;
- the first later passing observation on the same PR records `green_at`;
- normal blocked settlement and `PrMerged` events record the terminal
  milestones.

Expose the fold as a local store query:

```text
lf ci --since 7d

repo        wave            task     PR     fixes  detect  respond  green   merge   outcome
loopflow    infrastructure  W2-...   #...   1      1.2s    3.8s     11m     18m     merged
etude       rules           W2-...   #...   2      0.9s    4.1s     24m     —       green
cadenza     ear             W2-...   #...   1      1.1s    —        —       —       blocked
```

The summary reports, overall and by repository/Wave:

- CI incidents and distinct repair attempts per Task/PR;
- provider-to-Home detection latency and Home-to-Task response latency;
- failure-to-green, green-to-merge, and failure-to-merge duration;
- Task start-to-merge cycle time and the portion spent recovering CI;
- autonomous versus human-assisted recovery, derived from Human/Linear
  commands between trigger and green;
- unresolved incident count, age, current owner, and actionable blocker;
- ignored, unmatched, ambiguous, and duplicate webhook deliveries.

`--json` emits the same explicit DTO for weekly analysis. The command is
machine-wide by default because `lfd` and the registry are Home-level; `--wave`
and `--repo` narrow the view. It performs no GitHub reads.

## The smallest steering addition

Add one general command, not a CI-specific command:

```rust
ChildCommandKind::Trigger {
    identity: String,
    text: String,
}
```

A trigger means: **make the target aware of this external fact now**.

- If a provider turn is live and supports steering, deliver `text` as a live
  steer.
- If a turn is live but cannot steer, use the existing interrupt-and-replace
  fallback.
- If the Task is asleep, persist the trigger first and launch one body. A fresh
  failing `CiObservation` selects the existing `ci-fix` flow at generation
  birth; the trigger does not name or execute a flow.

The command is the durable wake receipt. It does not mint a Task directive.
A CI failure does not replace the Task definition, so `WorkRevised` and
directive acknowledgment are the wrong lifecycle. The persisted
`CiObservation` carries the typed PR/head/check evidence; the trigger only says
that evidence now requires attention.

For this adapter:

```text
identity = github:ci:<owner/repo>:<pr>:<head>:<failure-set-digest>
text     = CI failed on PR #<n> at <head>: <checks with log URLs>
```

The identity is semantic, not the GitHub delivery id. GitHub may send several
deliveries for the same failing head; the provider inbox deduplicates retries
of one delivery, while the command ledger coalesces all deliveries describing
the same failure set.

`ensure_child_trigger_command` mirrors the existing decision-command ensure
path. In one immediate transaction it finds an existing Trigger with the same
identity for the Task or inserts one. The existing command state machine owns
claim, delivery, uncertainty after crash, receipts, and relaunch. The existing
process lease remains the second exactly-one-body gate.

## GitHub webhook receiver

Extend `LfdState` with optional GitHub webhook configuration loaded from
`LF_GITHUB_WEBHOOK_SECRET`. Service files never contain the secret. When it is
absent, `/github/webhook` returns `503`; `/status` reports GitHub ingress as
`unconfigured` without exposing configuration values.

Add one route:

```text
POST /github/webhook
```

The route:

1. Enforces the existing 256 KiB body limit.
2. Requires `X-GitHub-Delivery`, `X-GitHub-Event`, and
   `X-Hub-Signature-256`.
3. Verifies the HMAC over the raw body before parsing it.
4. Records `(delivery_id, "github")` in `provider_deliveries` before any Task
   mutation.
5. Parses only `check_run` in this slice. Unknown events and non-completed runs
   are durably `ignored`; every completed run is a reconciliation nudge, and
   the aggregate Task observation decides whether CI is actually failing.
6. Returns `2xx` only after the delivery has a terminal inbox status and any
   routed Task trigger is durable. A store/routing failure returns `5xx`,
   leaving the delivery retryable.

Use GitHub's delivery id for the inbox key. Store event kind `check_run` and a
small outcome summary; never persist the raw signed body.

## Route and observe

A completed `check_run` supplies `repository.full_name`, PR number(s), head SHA,
check name, and log URL. Resolve it against open `TaskPr::pr_identity()` values:

- repository, PR number, and current head must all match;
- terminal Task predecessors route to their unique live successor;
- no match is `no_target` and visible in the inbox;
- more than one live match is an error, not a guessed route.

The webhook is a nudge, not the source of aggregate CI truth. After resolving
the Task, call the existing typed PR reconciliation operation to refresh the
full required-check set. Build the trigger from the resulting fresh
`CiObservation`, including actionable leaf checks and log URLs. If the current
head is not failing or the same failure set has already triggered, stamp the
delivery `processed` with `no_trigger`.

This keeps one GitHub interpretation in `ops::pr`/`ops::task`. `lfd` neither
shells out to `lf` nor grows PR business logic.

## Send and wake

The GitHub adapter calls one typed operation:

```rust
send_task_trigger(
    store,
    task_session,
    trigger_identity,
    trigger_text,
    TriggerIntent::CiFix,
) -> TriggerReceipt
```

`TriggerIntent` is an internal launch constraint, not persisted configuration
or a public trigger DSL. `CiFix` permits a sleeping Task with an open PR to
cross the normal open-PR restart bar only when its current `CiObservation`
matches the trigger identity. All terminal, abandoning, publishing, and stale
head bars remain.

The operation:

1. Ensures the Trigger command durably by identity.
2. Appends the existing `CommandChanged` event.
3. If a process is live, lets the normal command watcher deliver the trigger.
4. If no process is live, launches with the existing ci-fix restart intent.
5. Returns the command id/state for the provider delivery outcome.

Project supervision no longer calls `wake_task_ci_fix`. It may continue passive
PR reconciliation for its own view, but only `lfd` turns a GitHub delivery into
an automatic CI trigger. Standalone Tasks work because routing starts from the
PR receipt, not from a live Project loop.

During dogfood, both webhook and poll observations feed the same incident and
Trigger identity. The command ledger therefore prevents a double body while
`lf ci` shows which observer arrived first. Remove the Project supervisor's
automatic wake only after the live webhook path has recovered PRs across the
Home; passive PR reconciliation remains.

## Webhook setup

The first version keeps subscription creation explicit. A Home operator:

1. exposes the single `lfd` receiver through the maintained HTTPS boundary;
2. stores the shared webhook secret in Doppler as
   `LF_GITHUB_WEBHOOK_SECRET`;
3. configures each GitHub repository to send `check_run` events to
   `<home-url>/github/webhook` with that secret;
4. runs `lfd status` to confirm the GitHub receiver is configured and watches
   delivery counts/outcomes for the smoke test.

Automatic GitHub App installation and subscription reconciliation need their
own durable subscription model. They do not block proving the receiver and
trigger language and are not hidden inside this Task.

## Delivery order

This is three serial implementation slices, not one opaque change:

1. **Observe:** add `CiIncident`, `lf ci`, and milestone recording to the
   current poll-driven lifecycle. Capture the Home's baseline before changing
   behavior.
2. **Receive and send:** add the signed GitHub route and idempotent Trigger
   command. Run webhook and polling observation together through the same
   incident identity.
3. **Cut over:** dogfood on real Task PRs across the Home, attach the `lf ci`
   report, then remove Project-owned automatic wakes once webhook coverage is
   demonstrated.

Each slice leaves a useful inspectable system. A partially completed rollout
never makes broken CI less visible than it was before.

## Scope

In scope:

- GitHub secret/config state and `POST /github/webhook` in `lfd`.
- Signed `check_run` parsing and durable provider delivery receipts.
- Durable CI incident milestones and machine-wide `lf ci` reporting.
- Open Task PR routing from durable PR receipts across repositories and Task
  successors.
- Aggregate CI refresh through existing typed GitHub/PR operations.
- General idempotent `Trigger { identity, text }` Task command.
- Live-steer or sleeping-ci-fix delivery through existing command/lease paths.
- Removal of the Project supervisor's automatic CI wake ownership.
- Status and integration tests that expose delivery, target, trigger, and
  command receipt.

Out of scope:

- Restoring the old trigger registry, signal configuration, activation queue,
  event hub, scheduler, executor, or detached `lf` spawning.
- Generic provider plugins or arbitrary webhook-to-command configuration.
- Push-to-main, PR-merged, workflow-run, or deployment triggers.
- Automatic GitHub App installation/subscription reconciliation.
- Changing the ci-fix skill or its bounded settlement behavior.

## Operational proof

The primary evidence is the maintained Home running real repository CI, not a
simulated state transition.

Before cutting over wake ownership:

1. Configure the one `lfd` receiver for Loopflow and every GitHub-backed
   repository on this Home with an active Task PR during the trial.
2. Observe real broken-CI episodes in Loopflow and at least two other
   repositories. An intentionally broken check on a real Task PR is acceptable
   when organic failures do not cover a repository; the Task must repair and
   land that same PR through normal CI.
3. For each repository, show in `lf ci` that `lfd` received the failure, the
   owning Task responded, a repair produced green CI, and the PR merged. At
   least one incident per repository must complete without manual detection,
   `lf task resume`, or a replacement human steer.
4. Account for every incident during the trial as merged, green awaiting its
   normal merge gate, actionably blocked, or still open with age and owner. No
   silent/missing incident is acceptable.
5. Attach the machine-wide `lf ci --since <trial-start>` text and JSON receipts
   to the implementation PR. Report actual medians and tails for detection,
   response, green, and merge rather than inventing latency targets before a
   baseline exists.

The deterministic fixture suite remains the regression floor. It proves
signature rejection, durable-before-`2xx`, delivery and semantic deduplication,
stale-head rejection, crash recovery, exactly one body, live steering,
missing/ambiguous routing, and the absence of public exec/read routes. Passing
those tests is necessary; it is not the claim that the feature works.
