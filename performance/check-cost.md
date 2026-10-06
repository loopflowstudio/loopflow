# Check cost by step

```bash
uv run python -m scripts.check_cost --captures ~/.lf/runs \
  --since 2026-09-30T00:00:00-07:00 --until 2026-09-30T21:15:00Z

# Compare a later window after the revised skills are installed and used.
uv run python -m scripts.check_cost --captures ~/.lf/runs \
  --since 2026-10-01T00:00:00-07:00 --until 2026-10-01T14:15:00-07:00 \
  --baseline performance/baselines/check-cost-2026-09-30.json
```

Jack Heart requested this baseline on 2026-09-30 (LOO-357). The collector reads
existing local capture manifests, normalized command events and submitted context.
It emits aggregate numbers only. It neither invokes providers nor adds a store.

## September 30 baseline

Window: midnight–14:15 America/Los_Angeles, before this branch's implementation.
The [saved aggregate](baselines/check-cost-2026-09-30.json) includes all observed
skills and coverage counts. The selected lifecycle steps are:

| Step | Settled Runs | Recognized check commands | Check minutes | Check / Run time | Submitted tokens | Scratch tokens |
|---|---:|---:|---:|---:|---:|---:|
| implement | 27 | 108 | 45.5 | 7.0% | 2,202,173 | 520,826 |
| compress | 26 | 48 | 7.8 | 3.9% | 2,271,039 | 590,804 |
| realign | 4 | 1 | 0.1 | 0.1% | 114,942 | 82,659 |
| loop-decide | 1 | 0 | 0.0 | 0.0% | 22,240 | 19,924 |
| kickoff | 1 | 0 | 0.0 | 0.0% | 17,339 | 13,914 |
| design | 0 | unavailable | unavailable | unavailable | unavailable | unavailable |
| gate | 7 | 12 | 15.8 | 20.8% | 221,165 | 14,598 |

Implement's 1,628 shell commands occupied 261.1 minutes (40.1% of aggregate Run
time); 108 recognized check commands occupied 45.5 minutes (7.0%), with 18 nonzero
exits. Compress spent 7.8 minutes in 48 recognized checks. These categories
must stay separate: reads, edits and orchestration also use shell commands.
The Task's cited speed research (928 test commands / 7.9 hours on LOO-298 and
roughly 30% implement shell time) describes another population; it is not
recomputed or substituted for this day's baseline.

Implement scratch contributed 520,826 tokens, 23.7% of submitted context;
compress scratch contributed 590,804, 26.0%. Keyword-matched scratch paragraphs
contributed 123,540 words to implement and 135,144 to compress. This counts
paragraphs containing “proof”, “verification”, “verified” or “pytest”; it is a
context proxy, not a claim that every word was unnecessary. Repeated inclusion
across Runs counts repeatedly because the model receives it repeatedly.

## Where the habit came from

| Source at base 12013dae | Repeated obligation | Change |
|---|---|---|
| implement | Focused behavior plus all Done when checks; stop if required proof cannot run | Build/focused-test sanity; gate owns acceptance |
| compress | Requires runnable proof as input; stop on unavailable proof | Reuse results; check reductions only; defer unavailable environments |
| realign | Review evidence again, verify repairs, stop on unrunnable checks | Reconcile in place; one result line and no new verification pass |
| loop-decide | Required checks that cannot run explicitly justify Blocked | Missing display/human checks alone cannot block; preserve authored reviews |
| design / kickoff | Generate focused/end-to-end proof without naming its phase or runnable environment | Headless gate commands; judgment goes to demo/review |
| gate | Run suites, then a second verify-outcome section asks for more proof | Include acceptance in one planned run; assess the results afterward |
| LOOPFLOW.md | Universal evidence loop is paid on every Run | Compact phase ownership and headless rule |
| AGENTS.md | Implement owns both focused proof and Done when | Sanity now, acceptance at gate; one scratch result line |
| TESTING.md / hosted UI policy | Window captures called headless; required permissioned host and five-run quota | Deny WindowServer in default tests; optional native diagnostics |
| Intelligence memory | Historic proof limits repeatedly enter context | Preserve dated findings while explicitly removing implied per-pass obligations |

The authoring skill and PROMPTS.md also carry the headless-check rule so later
plans do not recreate the requirement. Historical research and published
receipts retain their original wording.

The seven lifecycle skills above contain 5,909 words at the base and
5,911 after this edit; their 41 occurrences of “proof” become zero.
This measures source text only, not installed skill adoption or saved runtime.

## Interpretation and next comparison

Command time is the union of observed start/completion spans within each Run;
parallel commands are not double-counted. Run times are summed, so simultaneous
Runs remain separate work. A shell batch containing a check includes that
batch's other work. Classification recognizes standard build/test/lint command
families, not arbitrary wrapper scripts. Heredoc batches and malformed shell
strings are unclassified: 336 implement commands and 65 compress commands in
this cohort. The recognized check times are lower-bound estimates; changes in
batching can change classification coverage. Quoted search arguments are not
executions. Reading source examples must not inflate the test count. Command
output and raw prompts never enter this report.

The cutoff excludes unfinished Runs from timing and context totals. Fifteen
Runs in the date window name repository paths that no longer exist; their
repository identity is unavailable and they are excluded, not assigned by name.
The report counts these separately. Existing checkouts are resolved through Git's
common directory. Zero recognized check time is not proof of no verification.
A comparison with missing command records or start times is omitted. Submitted
token counts exclude provider-native history, cache effects and later file reads.
Source changes are not a measurement of model behavior.

No post-change implement cohort exists yet. Compare settled implement Runs that
received the revised skills using the same collector, canonical repository,
command classifier, and time-window length. Report sample counts, recognized
check percentage and context alongside task mix; a difference is descriptive,
not a controlled causal estimate. Runtime improvement remains open until those
Runs exist. Do not rerun implementation solely to manufacture a favorable sample.
