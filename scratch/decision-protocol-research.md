# Decision protocol: lessons from Jev and Pydantic

Research on 2026-09-25, using the human's supplied Jev introduction and primary
Pydantic/TypeSafe documentation. Keep agent invocation through Loopflow's
existing Codex/Claude Code harnesses. This research does not authorize a direct
model API, new provider dependency, or Jev installation.

## Observed source claims

[TypeSafe's Jev introduction](https://typesafe.ai/blog/introducing-system-one-models-and-jev)
separates input state, predefined typed questions, and the surrounding workflow.
Jev returns bounded probabilistic judgments rather than arbitrary text. Its
workflow discussion favors decomposing questions and keeping branching in code.
The reported performance comparisons are vendor results under described
conditions, not evidence for Loopflow's workload. Its type-safety claim concerns
output shape; it does not establish that a chosen answer is semantically correct.

[TypeSafe's model limitations](https://docs.typesafe.ai/model-jaggedness/jev-1.13)
explicitly warn about literal wording, unrelated context, compound judgments,
indirection, and adversarial text. Deterministic counting and date comparisons
belong in code. Generation needs a generative model. These qualifications matter
for any future loop-decide implementation that must also explain a blocker or
write useful next-pass direction.

[Pydantic AI output handling](https://pydantic.dev/docs/ai/core-concepts/output/)
provides several output transports, including typed tool calls and native or
prompted structured output, with validation of the result. Output validators
can request correction. A final result mixed with other tool calls needs an
explicit settlement policy: accepting one result does not by itself prove all
concurrent side effects succeeded.

[Pydantic AI retry semantics](https://pydantic.dev/docs/ai/core-concepts/retries/)
distinguish transport retries, model/tool validation feedback, and whole-run
retries. Budgets at different layers can multiply. A model correction is not
the same operation as replaying a complete agent workflow. Human deferral is
control flow rather than a generic execution exception.

## Application to Loopflow (design inference)

Give loop-decide a small assessment context: accepted objective and completion
claims, previous pass direction, observed changes or learning, current review
findings, and relevant proof. Preserve references to the full artifacts for
inspection; do not replace required evidence with a confidence number.

Ask distinct questions in the skill: are this boundary's obligations satisfied;
what meaningful progress occurred; what specific next action remains; what
missing input needs a human? Code owns the available edge, pass budget, exact
invocation identity, and human authority. Keep Advance/Iterate as navigation
and Blocked as an execution outcome opening Ask. Do not add a matrix of stored
booleans merely to mirror prompt wording.

Use a typed result at the lf boundary, independent of provider transport.
The current CLI protocol is a practical adapter through either provider's tool
use. A future native structured-output adapter could feed the same validated
result without changing Flow semantics; it would need provider-specific proof
through lf's own harnesses. Native schema constraints were not found in the
current AgentConfig/harness search; do not claim they are integrated.

Malformed, missing, or conflicting output is a protocol failure with precise
feedback. A bounded correction attempt must not count as a new implementation
pass or replay implementation side effects. A valid Blocked judgment calls the
existing Ask system with unblock; a malformed answer is not evidence that work
made no progress. Never treat provider exit alone as Advance.

Record a candidate decision separately from successful Run completion. Settle
once only after both are present; a failed/interrupted Run discards its candidate.
Retain Ask identity and completion evidence so recovery joins the same human
conversation. A completed Ask supplies new evidence/direction, not a gate approval.

## Proof and future work

Fixtures should cover complete work, useful learning without code changes,
unchanged failure, missing evidence, malformed output, duplicate/stale results,
and human completion that leaves the blocker unresolved. Compare the eventual
judgments with human assessments. Do not describe self-reported Codex/Claude
confidence as calibrated Jev probabilities or tune a threshold without data.

Current scope keeps Codex/Claude Code invocation through lf. Jev remains a
future decision-agent option; it must not become a hidden dependency of this
branch. The runtime integration contract remains the implementation authority.

## Implemented application and limits

The CLI adapter and shared navigation enum now exist. The dedicated skill asks
for separate completion/progress/next-action judgments; its result supplies the
existing reducer. Rejected commands return errors to the same provider tool
loop. Missing output stops at that decision boundary. No extra automatic
whole-Run correction loop or new confidence/budget knob was added: explicit
retry revisits the failed boundary without incrementing the authored pass count.
The provider's existing turn limit remains the correction bound. Keyed Ask
retains completion across retry; human gates use a separate exact decision.
