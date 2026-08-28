# 5 Whys: Historical ledger gaps permanently gated daily telemetry

## The Problem

The 2026-08-20 `telemetry-daily` run stopped in `lf doctor` on eight
2026-08-04 through 2026-08-11 ledger gap-days even though no durable scheduler
obligation required a run on those days, so the telemetry scorecard never ran.

## Recorded Evidence

- Before PR #1248, `check_continuity` accepted only `run_events` and the current
  time. It treated every missing UTC date between the first and last observed
  event as a failure, and treated more than 24 hours without an event as a
  warning. It did not read cron installation or receipt evidence.
- The old unit tests made that proxy normative: a single internal gap-day had
  to fail, and a 29-hour silence inside two populated days had to warn.
- `.lf/flows/telemetry-daily.yaml` runs `doctor` before
  `__telemetry-scorecard`; any doctor failure prevents the scorecard step.
- The retained Infrastructure launchd jobs were created on
  2026-08-22T17:23:51Z. They durably name the wave, flow, fixed-daily schedule,
  and Home `home_39860354aaca640c2ccb50bf6ca609d8`, but the legacy files have no
  activation field.
- The earliest exact scheduled receipt for `infrastructure/telemetry-daily`
  began at 2026-08-22T17:24:01Z on that Home. Its target failed, but the receipt
  still proves the scheduler launched: `run_cron` persists the running receipt
  before placement validation or target execution.
- The eight August 4-11 dates therefore predate the first durable evidence of
  this Home's cadence by at least ten days. Their missing capture remains real
  historical evidence, but it is not evidence of a missed cron obligation.
- PR #1248 now persists or recovers activation, joins installed cron identity
  to exact scheduled receipts, judges only the latest due interval, and keeps
  raw gap-days diagnostic. Its copied-production-path test proves
  `telemetry-daily` reaches the scorecard without altering historical events.

## Chain

Permanent telemetry failure → every calendar gap was a failed continuity check
→ event presence stood in for expected scheduler execution → the scheduler had
no durable activation boundary exposed to doctor → tests verified the proxy
instead of the configured telemetry path → missing evidence was treated as a
missed obligation without first establishing the denominator

**Problem**: Eight terminal historical ledger gaps caused every later
`telemetry-daily` run to fail before its scorecard.

**Why 1**: `check_continuity` returned `Fail` for any internal calendar date
without a `run_events` row, regardless of whether anything was scheduled.

↳ *Could we have caught this earlier?* A test containing events before and
after a pre-activation gap, plus a current scheduled receipt, would have exposed
that old history could never recover.

**Why 2**: The check used general ledger activity as a proxy for scheduler
health. Its input had no installed cron, activation time, Home, schedule, or
receipt source from which to compute a real obligation.

↳ *What process allowed this?* The continuity check and cron runner were
reviewed as separate features. No configured-path test joined the cron's
declared cadence to the doctor gate inside `telemetry-daily`.

**Why 3**: The installed cron metadata described how to launch work but not
when that obligation began. Receipts described attempts after they happened,
but doctor never read either surface. There was no domain value corresponding
to “this Home owed this cron in this interval.”

↳ *What assumption was wrong?* A quiet ledger was assumed to mean the ledger
was unavailable. It can also mean that no owned work was due, that cadence was
not active yet, or that unrelated historical capture was lost.

**Why 4**: The original fix was shaped around a real 29.2-hour capture outage.
It generalized an observer-side symptom—elapsed time between rows—into a health
policy, and focused unit tests reinforced that heuristic without supplying an
authoritative denominator.

↳ *Why was that assumption encoded?* At the time, the observable event span
was the only readily available continuity signal. Cron later gained durable
receipts, but the monitoring model was not revisited to distinguish history
from current obligation.

**Why 5 (Root)**: Infrastructure health had no explicit expectedness rule.
Absence of an observation was allowed to become evidence of an outage before a
durable authority established the owner, activation boundary, due interval,
and receipt that should exist. That conflated historical missingness with a
currently violated obligation.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 1 | Why were the August 4-11 `run_events` absent? LOO-219 owns capture loss; its answer cannot change the cadence denominator. | High |
| Why 2 | What launched the 2026-08-20 telemetry run before the retained Home's 2026-08-22 cron activation? | Medium |
| Why 3 | How should a legacy installed job with neither a scheduled receipt nor trustworthy file birth time establish activation? | Medium |
| Why 5 | Do any other doctor or scorecard failures infer a missing obligation from observer silence instead of durable authority? | High |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Let terminal historical gaps remain diagnostic once the current due cron interval has an exact scheduled receipt. Shipped in PR #1248. | The August 4-11 gaps permanently blocking `telemetry-daily`. |
| Structural | Persist activation with the installed cron, preserve it across unchanged syncs, recover legacy activation from exact scheduled receipts, and compare the latest due interval with receipts matching wave, flow, target kind, Home, schedule, and scheduled source. Shipped in PR #1248. | Manual runs, target outcomes, stale identities, or pre-activation history being mistaken for scheduler continuity. |
| Systemic | Make negative health claims prove their denominator: durable authority, owner, activation, due interval, and evidence query. Without that proof, report history or unknown state rather than an outage. | Future monitoring gates turning absence of unrelated observations into permanent red state. |

## Changes to Implement

- [x] Restore cron continuity through durable activation and exact scheduled
  receipts while preserving historical events.
- [x] Prove the real `doctor` → telemetry-scorecard path on a
  copied-production-shaped store.
- [x] Audit remaining absence-based `lf doctor` and lifecycle scorecard gates.
  For each red state, record the durable authority, owner, expected interval,
  and evidence query; reclassify any claim without that denominator as history
  or unknown. Encode this rule in the health-check authoring guidance, without
  extracting a generic evaluator until a second concrete check needs it.

## Audit Result

Every remaining `lf doctor` failure reports either a direct invariant violation
or an error reading its named authority. Lifecycle scorecard rows already keep
missing samples and absent authority Unknown. One producer boundary still
synthesized an incomplete observed `0.0` for the undefined ratio `0 / 0` when
no settled Task was eligible. Emit the existing Unavailable observation with a
source-window reason instead; no new metric state or generic evaluator is
needed.
