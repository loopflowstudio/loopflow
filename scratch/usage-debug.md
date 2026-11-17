# Graph stopping — debug, 2026-11-13

Original run facts and exact quotes remain in usage-drain.md; no affected
workflow was relaunched. EXP-56's pass count still does not establish futile
review. MTG-338's recorded pass-10 decision does establish that another negative
availability observation was used to justify restoring the same candidate.

## Cause and competing explanations

- A missing iteration check is real but intentional, not a broken comparison:
  graph/transitions.rs tests iteration even at u32::MAX. Counters observe
  backward edges; they do not limit them.
- An ignored blocked verdict is not the observed failure: MTG-338 returned
  iterate. Decoding, settlement and reporting already support blocked, retaining
  its reason and stopping before more work. GraphHeld exits 3; the Task carrier
  retries only exit 1, up to three attempts.
- Conflicting decision instructions are established. The captured incident
  prompt said “new evidence counts as progress” and “when a person must resolve
  the question”. Current source has those phrases. loop-or-next permits stopping
  for no progress but says unavailable checks never block; optimize-plan says
  blocked only for a failed harness/workload; batch stand-ins say there is no
  one to hand a blocker to. Reobserving an unchanged obstacle could satisfy
  this loose progress definition while the stop option appeared unavailable.
  This explains permission to repeat, not a proof of the model's reasoning.
- No cumulative Graph token, cost or elapsed-time enforcement was found in
  runner, Task carrier, configuration or step launch. Context budgets measure
  authored input size. Provider-limit recovery handles refusal/reset and can
  restart eligible Tasks; it is not a proactive spend ceiling. Output correction
  is bounded to two turns; failed Graph attempts to three. Neither bounds a
  successful sequence of iterate verdicts. Interrupts and interactive stops
  work independently.

## Targeted repair and its limit

Align shared output, managed decision context, batch stand-ins, chat Graphs and
builtin deciders: unchanged obstacles do not justify another pass; required
unavailable results can stop this step, while checks assigned to later steps
remain deferred. A blocked answer reports partial results, unverified work and
what must change before another attempt.

This is a prompt-contract repair, not a hard usage ceiling or a demonstrated
elimination of model-driven repetition. Captured Task definitions and customized
skills can retain old text; this checkout does not update those or installed
binaries. No blanket model downgrade, external message or live Task mutation.

## Decisions for launch-plan

1. Select one initial enforceable bound and its unit: successful backward
   traversals are cheap and deterministic; token/cost limits need receipt,
   cache and provider semantics; elapsed time also charges useful long waits.
2. Define scope/reset across overlapping and nested edges, failed Graph
   attempts, Task runs and automatic restarts. A per-process limit can be
   bypassed by starting a fresh process.
3. Decide how a limit stops: between steps versus interrupting a running agent,
   how partial work is reported, and what authorizes another attempt. Do not
   represent incomplete required work as advance (which may reach publishing).
4. Stop coverage: GraphOutput supplies decisions only at steps with backward
   edges. optimize first-plan is ordinary; successful prose saying no experiment
   can run is followed by implement. Decide whether ordinary steps need a stop
   result or topology should provide an initial decision. The current repair
   does not change this behavior.
5. Review churn needs an in-scope defect and useful next action, but a fixed
   number of reviews alone cannot distinguish useful fixes from scope growth.
   Inspect EXP-56's findings before assigning its repeats a cause.

## 5whys — systemic cause, 2026-11-13

Status: containment complete; recurrence prevention incomplete. MTG-338 spent
shared Codex capacity restoring an untested candidate without obtaining its
required measurement. The recorded cohort usage in usage-drain.md establishes
volume, not how much quota this loop consumed or who else lost access. This
analysis reuses those observations; it did not reread the Session cohort or
restart work.

The causal chain has two distinct gaps:

- **Why another pass?** Pass 10 explicitly treated another negative availability
  observation as progress. The captured instructions permitted that reading
  while narrowing when stopping was allowed. The current diff repairs those
  contradictions; it cannot establish why the model chose that reading or
  guarantee a different choice next time.
- **Why could that choice keep spending?** Successful decisions enter ordinary
  routing, not failed-attempt recovery. `GraphOutput::decode` validates a decision
  and nonempty text, not usefulness, scope or affordability. `finish_step` then
  follows the backward edge. The same agent judgment that proposes more work
  also supplies the permission to repeat; there is no independent resource
  stop in that transition. Existing failed-attempt and output-correction limits
  therefore do not constrain a stream of valid iterate answers.
- **Why don't counters stop it?** They were built as observations. One precision
  correction: `ExecutionCursor::finish_inner` does check u32 iteration overflow.
  “Unbounded” here means no practical resource budget, not infinite traversals.
  An overflow stop after billions of repeats cannot protect usage, and a repeat
  count does not bound spending inside one step.

Review expansion is a separate plausible mechanism, not an established EXP-56
diagnosis. Current `pursue.yaml` has overlapping returns to implement; later
reviews replay compress, realign and earlier reviews. Compress reaches related
callers and concepts; concept-review can revise the design and request another
implementation. Each pass can thus make useful local changes while generating
fresh obligations. Scope is constrained in prose, but the routing contract does
not distinguish a required defect from an optional improvement. This is a route
to a moving finish line, even without an unchanged obstacle. Counterevidence:
concept-review separates open product choices, loop-or-next requires accepted
scope, and EXP-56's sampled finals include concrete fixes and advance decisions.
Its captured Graph and successive findings would need comparison before blaming
that incident on scope growth; today's builtin topology alone cannot do that.

Prevention still proposed, for launch-plan to select:

- Pair the prompt repair with an enforced allowance whose exhaustion stops
  before more work and reports partial results. Define ownership and reset
  across nested edges, retries and subsequent runs. A repeat allowance bounds
  cycles, not tokens; a usage/time allowance needs its own measurement and
  in-flight stop semantics. Do not claim one provides the other's guarantee.
- Make review direction distinguish unmet requirements from proposed expansion,
  so an optional improvement cannot silently become required implementation.
  Check both a real defect that merits another pass and a worthwhile addition
  that should remain outside the current work.
- Exercise the complete stop path with always-iterate outcomes, nested loops,
  failed attempts and a subsequent run: allowance cannot silently replenish,
  partial work remains inspectable, and publishing never follows exhaustion.
  Include the ordinary first-plan gap above. Separately evaluate repaired
  decisions against the recorded unchanged-obstacle case; scripted blocked
  output tests only prove that an already-chosen stop is honored.

No additional runtime policy was implemented here. The existing 99-test result
belongs to debug; this source-and-diff review added no test run. Next: launch-plan
selects the end-to-end prevention and its acceptance checks in this checkout.

## Verification

- With LF_* unset and stdin closed, cargo test -p loopflow --lib graph:::
  99 passed, 0 failed (2026-11-13). Includes structured blocked output after one
  repeat, retained partial-result reason, no publish, nested loops, interrupt
  handling and unbounded-repeat behavior. Scripted outcomes validate mechanics,
  not whether a model will now choose blocked in the original incident.
- cargo fmt: passed. git diff --check: passed.
- Build emitted a macOS linker warning: __eh_frame exceeds the 16MB compact
  unwind offset limit. Tests completed successfully.
- Gate/before commit: cargo clippy --all-targets -- -D warnings remains to run.
  No server, watcher, provider experiment or background job remains from debug.

## Execution decision — launch-plan, 2026-11-13

Keep one implementation here: a persisted repeat allowance plus a stop result
at every agent step. Ship both with the existing stopping-contract repair.
This section is the implementation design; **the runtime facility is not yet
implemented**. No Task, Wave, Graph or affected workload is launched.

### Allowance and ownership

Start with **three backward traversals total**, in addition to the initial
forward pass. Each settled Iterate consumes one, regardless of edge length.
All nested bodies, XOR paths and overlapping edges share the same allowance;
returning to an earlier body never creates another allowance. Forward work,
including the final review, can finish with zero remaining. A fourth Iterate
stops before its target launches. This bounds repeats, not tokens, cost, wall
clock time, or work inside one agent or command.

For managed work, the Task owns `repeat_limit` and `repeats_used`, independent
of GraphProcess lifetime. Retries, provider recovery, interactive stops, plain
subsequent runs, `--from`, graph replacement and shipping retain both values.
A different graph does not replenish a Task. Task-associated auxiliary Graphs
charge the same owner. This deliberately avoids a new execution-lineage model.
For a standalone Graph, its root execution owns the allowance; nested execution
inherits it. A separately requested standalone invocation starts with three.
That is a documented escape from a per-invocation bound, not a claim to enforce
a Machine-wide budget or prevent agents launching unrelated work.

Use one persisted allowance in existing execution storage, not a sum of
per-edge counters: child counts are copied into parents and may be reconstructed.
Keep those counters for display. Settle consumption and the corresponding cursor
transition atomically, keyed by the existing boundary identity. Recovery of an
already-settled result must neither charge twice nor refund it. An attempted
repeat whose work later fails stays charged; failed-step retries use the existing
three-attempt policy and cannot reset the repeat allowance. A failed persistence
write starts no next Graph step; it must not interfere with an active provider
conversation. Serialize grants and settlement against the same owner.

For existing Tasks at upgrade, initialize used from recorded settled backward
transitions where recoverable, and limit to max(3, used), giving no fresh repeats
to work already over three. If history cannot establish used, initialize with no
remaining allowance and explain why; forward work can still finish. New standalone
executions use three. Implement storage conversion explicitly, not DTO defaults.

### Stopping and continuing

Exhaustion is a held Graph outcome (exit 3), never Advance or an automatically
retried failure. Persist the pending Iterate and its direction without moving
the cursor. History and status show the step, used/limit, last result, checkout
and Session links, remaining unverified work where reported, and how to continue.
Render this from saved results; do not spend another agent call summarizing it.
Where a result omitted validation details, say they are unknown. Keep edits and
artifacts in place; do not publish, merge, revert, or mark the Task complete.

Proposed interface: `lf task run ISSUE --add-repeats N`, positive integer only,
adds N to the lifetime limit and then continues. Reuse existing run selection
for the starting position. A plain run at an allowance hold returns the same
report without launching an agent. Consume the retained pending Iterate once
after a grant; do not rerun its decider. A new GraphProcess carries continuation;
the stopped process stays history. `--ship`, batch mode and provider reset never
grant repeats implicitly. The running Graph's agents must not grant themselves
more repeats: do not teach or automatically invoke the grant in headless
stand-ins. This is operator control, not protection against arbitrary shell code.
A standalone invocation may select an initial `--repeat-limit N` (including 0);
there is no unlimited setting in this core.

### Ordinary steps can stop

Extend the captured output contract to every managed agent skill: ordinary
steps return `decision: advance|blocked`, with summary/reason following the
existing decision shape. Iterate remains valid only with a backward edge.
Routers gain a blocked alternative alongside path selection. Decode these into
the existing blocked outcome; persist the reason before returning exit 3.
Commands keep their existing exit/outcome handling. No prose matching, extra
checker agent, or optimize-only topology workaround.

Thus optimize's first-plan can report unavailable required measurement and stop
before implement. Apply the same contract to interactive settlement and batch
stand-ins. Plain skills outside a managed Graph keep ordinary answers. Chat-only
Graph instructions can teach the allowance and stop protocol, but have no runtime
authority to enforce it; documentation must distinguish those executions.

### Implementation and acceptance

Implement ownership/storage and atomic settlement first, then runner enforcement,
output capture/decoding, continuation flags and status. Keep them in one useful
change. Update orchestration docs, command help, persisted/wire fixtures and
consumers together; replace the current “no pass limit” claims when enforcement
lands. Put the shipped contract in docs/orchestration.md before scratch cleanup.

Done when isolated tests demonstrate:

- Always-Iterate executes initial work plus three repeats, holds on the fourth
  request, retains partial results, and never reaches publish. At zero remaining,
  Advance can still finish normally.
- Mixed nested, overlapping and reentered XOR edges spend the same three units;
  unrelated Tasks and independently requested standalone Graphs remain separate.
- Crash before/after settlement, replayed receipts, failed-attempt recovery,
  provider reset, a subsequent run, `--from`, graph replacement and shipping do
  not replenish allowance or double-charge. Concurrent settlement/grants preserve
  the count. Upgrade fixtures cover known and unavailable historical counts.
- Plain continuation at exhaustion launches nothing; adding two allows exactly
  two more traversals, uses the saved direction once and leaves old processes
  stopped. Zero/negative additions fail without mutation.
- Ordinary first-plan and XOR-router blocked results stop before downstream work;
  reasons survive process restart. Invalid output uses the existing bounded
  correction path and cannot become a successful Advance.

Run focused graph and Task execution tests with isolated LF environment and
stdin closed; then formatting, clippy and affected gate suites. Check new owner
queries against a representative 10,000-Task store with EXPLAIN QUERY PLAN.
Separately evaluate repaired prompts on a frozen unchanged-obstacle transcript
and a real repairable defect; scripted stop tests do not establish model judgment.
Do not restart either incident workload for validation.

### Product choices and deferred work

Three is a conservative initial policy choice, not a value established by these
incidents. Sharing it across a Task's entire lifetime may stop useful later
reviews; explicit additions are the selected tradeoff. Revisit the default and
Task-wide scope after observing legitimate holds, without weakening persistence
in this implementation. Record these assumptions in questions.md.

Defer token/cost/time ceilings and in-flight cancellation, independent allowances
for newly selected Graphs, and automatic grants. They need different accounting
or reset semantics. Keep review-scope expansion investigation separate: EXP-56's
repeat count alone does not establish that cause. No model-selection work.

Launch-plan check: source and current diff reviewed; git diff --check passed.
No runtime tests rerun for this design-only step. The 99 passing graph tests above
cover the existing patch, not the proposed allowance or ordinary-step contract.
