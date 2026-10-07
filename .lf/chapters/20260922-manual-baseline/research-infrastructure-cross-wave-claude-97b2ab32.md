# Research: Infrastructure's execution redesign and its cross-wave effects (through 2026-09-22)

## Scope and evidence boundary

This report tests one interpretation: that Infrastructure's main summer-2026
execution story was the topology shift from clients calling `lfd` over HTTP
toward one complete `lf` executing locally on each sovereign Home, reached
remotely only through explicit `lf ssh` transport, with the Work/execution
split as the semantic contract making that viable — and that much subsequent
runtime work was hardening that distributed topology.

Sources read on 2026-09-22: `lf activity --since 120d` per Wave and per Task,
`lf status {infrastructure,product,intelligence} --json`, `lf roadmap`,
`wave/{infrastructure,product,intelligence}/MEMORY.md`,
`docs/architecture/{execution,homes}.md`, and the baseline KR ledger in
`scratch/chapter-review.html`. No git or GitHub history was consulted.

Hard limits on this evidence, stated up front:

- The activity surface caps at 200 rows per query. The product feed truncates
  at 2026-07-15; June and early-July product receipts are not observable here.
- `lf activity --wave intelligence` mixes in a *different repository's* Wave of
  the same name (ETU-* tasks, PRs #110–#178). Wave-name scoping is not
  repository-scoped on this read surface. Every ETU row was excluded.
- `lf activity --task LOO-129`, `LOO-271`, `LOO-272`, and `LOO-143` return
  **zero receipts** — no created, started, or merged facts — even though their
  Linear task bodies assert "merged LOO-129/PR #1240" and "merged LOO-271/PR
  #1241" and Linear marks all four complete. Without git access I cannot
  independently confirm those merges. They are treated below as
  Linear-asserted, receipt-absent.
- No chapter start snapshot exists (the ledger itself says so).

## The record, Wave by Wave

### What Infrastructure's own receipts show

The infrastructure feed (200 rows, truncated, 2026-07-20 → 2026-09-22) is
dominated by two bursts and one long quiet:

- **2026-07-20/23:** a dense repair burst — Task lifecycle atomicity (LOO-1,
  LOO-2, LOO-3, LOO-206, LOO-209, LOO-211), Wave start/wake lifecycle
  (LOO-102), release publisher and cron-host bootstrap (LOO-212, LOO-214,
  LOO-215, LOO-37), architecture map (LOO-208), repository-owned Wave moves
  (LOO-127), performance scorecards (LOO-216), telemetry actionability
  (LOO-219).
- **2026-08-19/28:** the correction wave — status distinguishes last-recorded
  from current (LOO-229, PR #1225), main agents off canonical main (LOO-253,
  PR #1230), Task promotion recovery from live process truth (LOO-265,
  PR #1237), ledger continuity matched to scheduled obligations (LOO-241,
  PR #1248), Home-upgrade idempotency (LOO-228/264), local installs without
  destroying live work (LOO-226, PR #1229).
- **2026-09-01 → 2026-09-22:** near-silence. Two release-repair Tasks
  (LOO-259, LOO-275) on Sept 1, then nothing until LOO-279 (Linear OAuth
  refresh for unattended Project runs) on Sept 22.

Infrastructure's three current Projects are stability-security,
technical-architecture, and performance-efficiency. The Home/topology work
that exists in its roster: LOO-60 ("Loopflow has no stable machine identity;
gethostname is not one"), W2-88 ("Make Wave home an explicit inherited SSH
target"), LOO-91 ("Resolve Session bodies through the current Home lf"),
LOO-92 ("Reintroduce lfd as the Home subscription ingress"), LOO-101
("Discord authority survives Home and SSH boundaries"), LOO-127, LOO-102,
LOO-220's sibling release-Home work, and the architecture-map Tasks (LOO-208,
LOO-270) whose checker now passes 19/19 first-parent landings (the ledger's
one clean Infrastructure "holds").

### What Product's receipts show

Product's `loopflow-api` Project contains the execution redesign's core, by
receipt: LOO-196 "Delete Session and make Run the sole executor" (PR #1099,
2026-07-18), LOO-194 "Recover a Run across accounts and providers" (PRs
#1098/#1100, same day), LOO-207 "Make managed Tasks survive missing lifecycle
flows" (PR #1228), LOO-225 "Make Task lifecycle selection match the work it
must finish" (`lifecycle-capability-contract`, PR #1207), LOO-237 "Carry Run
execution context across User, Wave, and Project runners" (PR #1214), LOO-238
"Keep concurrent trace capture from killing Task Runs" (PR #1226), LOO-248
"Make PR landing a watched CI-repair loop" (PR #1231), LOO-162 "Make
LLM-session checkpoints authoritative for Task landing" (PR #1233), plus the
Turn-Basis series (LOO-256, LOO-260, LOO-269) and LOO-220 "Make release
upgrades converge on an always-on Home" (PRs #1186/#1189).

Product memory additionally records, without an identifiable Task receipt in
the observed window: the deletion of the Mac's HTTP-to-lfd data path
(`LocalWaveService` ~1500 lines, `WaveServiceProtocol`, ~22 consumers moved to
`RegistryQuery` shelling `lf --json`), the "reads never block on lfd"
invariant, and the PR #1250 Sessions contract (settled 2026-08-30 — a date at
which the product wave's activity feed shows no receipts at all).

Product's mac-surface-ux Project — the Wave's actual surface bet — has its two
largest open Tasks incomplete: LOO-251 (Sessions multiplexer) and LOO-274
(restore Wave fleet/roadmap loading in the Mac app). All 7 of its KRs are
unverified (`holds=false`).

### What Intelligence's receipts show

The Intelligence Wave record in this repository was **created 2026-08-19**
(`lf status`: `created_at 2026-08-19T23:28:40Z`; activity: "Wave intelligence
created", Project `trace` 08-20, Project `context` 08-22). The memory file
says the wave was "renamed from `memory` in the 2026-07-08 restructure," so
the operating context is older, but as durable registered Work, Intelligence
existed for roughly the last five weeks of a four-month chapter.

Its adaptation work, per Task contracts: LOO-128 ("Make every run a complete,
inspectable local record", started 08-20) narrowed into LOO-246 ("Make trace
audits survive persisted capture schemas" — motivated by **1,767 capture
integrity failures** on the fully-migrated real ledger), LOO-129 (strict
replay preflight, asserted PR #1240), LOO-271 (immutable pre-launch
ExecutionContracts, asserted PR #1241 — motivated by a 2026-08-23 audit
finding **0/10 recent invocations strictly replayable**), LOO-272 (unattended
replay executor), LOO-143 (inspectable launch context). Linear marks all of
these complete. The activity surface holds receipts for none of the last four,
and LOO-246's condition is currently `blocked` — its worktree is missing and
its PR record sits in `working` phase despite Linear completion.

All 5 Trace KRs and all 6 Context KRs read `holds=false`. The baseline ledger
is blunter: "Trace: six settled records lacked context, 35 lacked a launch
contract, 21 lacked token evidence, 76 lacked cost, and no replay launches
were recorded. 5/5 KRs do not hold." Its one strong positive is Context's
20/20 exact launch-context reconstruction.

### The September stall (all three Waves)

On 2026-08-28 all Project runners failed identically: "Project plan refresh
blocked before the next phase: could not refresh wave/&lt;name&gt; from Linear:
Stored linear token expired and automatic refresh failed" (product 22:01 and
22:20, intelligence 22:01, per `lf status` `last_failure` rows). No Project
runner ran again before 2026-09-22, when LOO-279 was filed to fix unattended
Linear OAuth refresh. Nearly a quarter of the review window's controller-driven
work simply did not happen, blocked by one centralized credential dependency.

## The five questions

### 1. Product work that implemented, projected, or repaired the execution model

Observed: the execution model's *semantic* core — Run as sole executor, Run
recovery across providers, lifecycle capability contracts, explicit execution
context, telemetry demoted from control plane, watched landing, Turn-Basis
control authority — is recorded almost entirely as Product `loopflow-api`
Tasks (list above, ~14 Tasks with merge receipts between 07-17 and 08-21).
The *projection* side — Mac reads converging on `RegistryQuery`/`lf --json`,
deletion of the HTTP-to-lfd data path, the Sessions projection (PR #1250),
truthful attempt-failure presentation, the `lf stop` verb — is recorded in
Product memory. Counting the loopflow-api roster: of its ~60 completed Tasks,
the clear majority are runtime/execution semantics; perhaps a half-dozen are
what the Project's own definition calls its bet (the shared product contract
across CLI/Mac/iOS surfaces).

Interpretation: Product didn't just *consume* the new execution model; it
built most of it. "Infrastructure's redesign" is a misnomer at the level of
recorded Work attribution.

### 2. Intelligence adaptation — and failure to adapt

Adapted (observed): the ledger contract itself predates the chapter's runtime
churn (post-057: trace=run_id, span=process_id, cumulative-usage rule,
`lf usage` reading the ledger directly after the `lfd::client` module died
with its only consumer — an early, real instance of the lfd-demotion pattern).
In the last five weeks, Intelligence produced the replay-contract stack
(LOO-129/271/272) and the audit-survival narrowing (LOO-246), which is
adaptation *to* the new model: content-addressed pre-launch contracts, Home
artifact authority, clean-commit binding, typed refusals instead of ambient
reconstruction — the same authority discipline `execution.md` mandates.

Failed or lagged (observed): every Trace KR is unproven; the migrated real
ledger threw 1,767 capture-integrity decode failures at `lf doctor`; the
baseline found settled records missing context/contract/tokens/cost and zero
replay launches; the Infrastructure summary reports **all 35 observed
scheduled telemetry runs failed**; the 29.2-hour silent ledger outage and the
fresh-db-blindness constraint (CI green while the only real-history machine
was broken) are recorded as standing constraints, not closed. The migration-061
collision (product `061_pm_snapshots` vs intelligence `061_trace_capture` on
shared `lfd.db`, plus an in-place edit of applied migration 057 taking down
every command sharing the db) shows Intelligence's capture schema and
Product's PM schema still colliding inside the one store the decentralization
never dismantled.

One more failure mode is self-referential and observed directly in this
research: the trace Project's own delivery is invisible to the durable
activity record (zero receipts for LOO-129/271/272/143). The observation layer
cannot currently prove the observation layer's own work happened.

Interpretation: Intelligence adapted its *contracts* to the new execution
model late but coherently; it did not adapt its *operating evidence* — the
readers, audits, and scheduled monitors run red or silent against the runtime
the redesign produced. The runtime's boundaries moved faster than the layer
meant to join them, and Intelligence had only five weeks of registered
existence in which to catch up.

### 3. KRs and Tasks displaced by the redesign

Observed:

- Product Projects **wave-chat**, **ios-surface-ux**, **distributed-computing**
  and **product-performance** were present in an earlier product portfolio and
  are absent from the 2026-07-21 and current snapshots; the baseline ledger
  notes durable Project Work still exists for `product-performance`,
  `wave-chat`, and `release-stability` with no owning snapshot entry. The
  Mac-surface KRs (paint budgets, control-room paths, embedded-terminal
  latency) have no observations; LOO-251 and LOO-274 sit open while the wave's
  August merges were nearly all runtime repairs (LOO-237/238/247/248/249/250/252
  in the same four-day window).
- Intelligence's dashboard "movement metrics" remain explicitly parked
  "until a delivery record and `escalated` exist" — records the runtime
  redesign was supposed to yield and has not. Evals was retired 2026-07-10 by
  Jack's decision — before the July 18 convergence, so not displaced *by* the
  redesign, but its revival condition ("delivery, intervention, complete
  context, transcript evidence") is exactly the evidence the redesign era
  failed to produce, so it stays parked.
- The September stall displaced everything controller-driven for ~25 days
  across all three Waves (observed via the uniform 08-28 `last_failure` rows
  and the empty September feeds).

Interpretation: the clearest displacement is inside Product — surface work
yielded to runtime repair for most of August. The September displacement is
*not* attributable to the distributed redesign; it is the residue of a
centralized dependency (Linear OAuth) the redesign never touched. Task-loop
trust KR: "metric exists but has no observations; freshness recorded as
never" — displacement of measurement itself.

### 4. Work misclassified by Wave

Observed, by roster inspection:

- **Trace/ledger work filed under Infrastructure:** LOO-18 (deterministic
  concurrent ledger writes), LOO-45 (performance/cost visibility), LOO-53
  (ops telemetry migration), LOO-63/70 (retiring parallel spend recorders and
  usage parsers), LOO-82/83 (trace lineage and capture-reference survival),
  LOO-106 (partial trace capture accumulation), LOO-219/241 (telemetry
  continuity) — all sit in Infrastructure's performance-efficiency and
  stability-security Projects. This is Intelligence's stated mandate
  ("Intelligence owns Context and Trace").
- **Execution-model work filed under Product:** the entire §1 list. LOO-238
  (trace capture must not kill Task Runs) is simultaneously trace-domain and
  runtime-domain and landed in Product.
- **Near-duplicate split across Waves:** LOO-237 (product, "Carry Run
  execution context across User, Wave, and Project runners", PR #1214) and
  LOO-243 (infrastructure, "Carry Run execution context across async Work
  runners") are the same concern divided by Wave boundary.
- **The catch-all is named by the review itself:** the baseline ledger states
  three current Tasks "do not fit cleanly inside their owning Project" and
  calls Loopflow API "a catch-all."
- **Read-surface misattribution hazard:** `--wave intelligence` returning
  another repository's ETU-* receipts means any cross-wave accounting done
  naively over `lf activity` would credit Intelligence with ~25 merges it does
  not own in this repo.

Interpretation: the Wave boundaries predate the architecture that emerged.
"Infrastructure" ended up meaning *operations* (releases, crons, credentials,
worktrees, promotion), "Product" absorbed *runtime semantics* because the
runtime is the product contract, and "Intelligence" was re-founded mid-chapter
to own what both had been leaking. The misclassification is systematic, not
occasional.

### 5. Does the record support the interpretation?

**The topology claim is supported as a description of the end state.**
`homes.md` and `execution.md` state it verbatim: "`lf ssh` is transport, not a
second API"; "no implicit fan-out and no central Run database"; one Skill →
one provider result → one Home-local Run record; manifest-before-spawn;
no `owner.json` → no durable cross-process signal authority. Infrastructure
Tasks (LOO-60, W2-88, LOO-91/92/101/102/127) and the architecture map that
passes 19/19 landing checks are real Infrastructure contributions to it. The
Work/execution split as the enabling semantic contract also matches the
record: stable Work surviving process replacement is what let `lfd` shrink to
a Home service keeper.

**The attribution claim is not supported.** "Infrastructure's main summer
execution story" was mostly written in Product's task roster; the Mac-side
demolition of the HTTP data path lives in Product memory; Infrastructure's
own receipt volume is dominated by release/cron/credential/worktree/promotion
hardening. If the interpretation is about *whose Work record* carries the
shift, it names the wrong Wave.

**"Much subsequent runtime work was hardening that distributed topology" is
half-supported.** The August correction wave (status truth, process-truth
recovery, capability contracts, worktree isolation) genuinely hardens the
distributed model. But a comparable volume of subsequent work hardened things
orthogonal to topology: release retries, Doppler bootstrap, migration
frontiers, Linear team migrations, CI-fix lifecycles. And the two worst
late-chapter failures are *centralization* residue, not distribution
hardening: the shared `lfd.db` migration collisions and the Linear OAuth
expiry that stalled every Project runner for three-plus weeks.

**Competing explanation the record supports at least as well:** the
through-line is not topology but *authority separation after impersonation
failures* — recorded state vs current truth (LOO-229), telemetry vs control
(LOO-238/265/241), placement vs identity (LOO-253, Home model), containment
vs progress (LOO-207). Decentralized topology is where that rule's
consequences landed, because only the local actor can prove its kind of fact.
The topology framing also cannot explain why the chapter's ending is
dominated by two centralized single points (Linear, shared `lfd.db`) that the
"sovereign Home" story leaves untouched.

## Observations vs interpretation

**Observed** (receipt-, contract-, or file-backed): every dated Task/PR pair
cited above; the Wave creation dates; the 08-28 triple runner failure and
empty September feeds; the KR `holds` values; the baseline ledger's counts
(39 KRs reviewed, 2 hold / 16 do not / 24 unknown per its filter chips — the
chips sum to 42, three above the stated 39, an internal inconsistency of the
artifact itself); the 1,767 capture failures and 0/10 replayability audit (as
quoted in Task contracts); the architecture docs' topology contracts; the
empty activity results for LOO-129/271/272/143; the ETU-* wave-name
collision.

**Interpretation** (mine): that Product built most of the execution model;
that Intelligence adapted contracts but not operating evidence; that Wave
misclassification is systematic; that authority-separation explains the
record better than topology; that the September stall is centralization
residue rather than redesign cost.

## Open unknowns

- Whether PRs #1240/#1241 and the LOO-272 replay executor actually merged:
  Linear asserts it, Task descriptions assert it, no activity receipt exists,
  and git was out of bounds for this report.
- Which Task (if any) owned PR #1250 and the LocalWaveService deletion; both
  are memory-recorded only.
- Product receipts before 2026-07-15 (feed truncation) — the June
  HTTP-to-lfd baseline rests on Product memory's dated "verified 2026-06-30"
  patterns section, not on receipts.
- Why completed Linear Tasks (LOO-246) retain `working`-phase PR records and
  missing worktrees — completion path or record, one of them is wrong, and the
  read surface cannot say which.
