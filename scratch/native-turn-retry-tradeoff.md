# Native turn attribution: bounded tradeoff review

LOO-298 · 2026-09-28 · Prepared for Jack Heart; subsequent supervisor disposition below.

Supervisor selected the bounded experiment under Jack's existing autonomous
implementation direction (Task comment `c68eb6fb-3516-4da2-b2ed-62626aeab23b`).
The ready consultation was completed and disappeared from the unresolved list.
This is not a new approval attributed to Jack and does not select a retry-contract
change. The original recommendation and observations below remain evidence.

Recommendation: permit one bounded native handoff experiment before changing
the accepted automatic-retry contract. This is a recommendation, not recorded
authorization from Jack. Retain one caller-token creation point and the shared
navigation check. No executable changes were made by this review.

## Observations

Inspected the working diff and both retained results for candidate
`d2791ad2264bde9389b3a9f66b6d44d1cde4d01ac54965b4e7071667b0b6413e`:

- `.lf/tmp/execution-model/native-turn-caller-late/results.json` rejects the
  failed turn's delayed child and leaves the decision boundary unresolved.
- `.lf/tmp/execution-model/native-turn-caller-replace/results.json` also rejects
  the authorized successor's Advance. Failed and successful provider turns remain
  recorded, but the Flow cannot consume the successor. This is a failed repair.
- `.lf/tmp/cut-i/codex-schema/v2/TurnStartParams.json` and
  `ThreadSettingsUpdateParams.json` offer no general config/environment override.
  `ThreadResumeParams.json` offers config; the retained live-thread test shows
  that supplying it did not refresh the tool environment in this path.

The [official app-server documentation](https://learn.chatgpt.com/docs/app-server#unsubscribe-from-a-loaded-thread),
read during this review, describes last-subscriber unload after 30 minutes without
activity. Unsubscribe is not evidence of an immediate reload. Archive changes
persisted visibility and attempts to archive descendants; it is not a suitable
transparent retry primitive without a separate design.

## Proposed experiment and stop rule

Use disposable native fixtures to establish whether the existing conversation
handoff/replacement mechanisms can resume the same persisted thread with fresh
caller identity. First establish exclusive conversation ownership; a second
writer on the same thread is not a handoff. Preserve an unrelated live sibling.
Do not kill a shared engine or use archive/unarchive as a reload shortcut.

One candidate must pass the paired tests: reject the old delayed child, accept
the legitimate retry decision, retain AgentSession/thread and both outcomes,
consume only the successor once, and leave a shared-engine sibling intact.
Keep the ordinary retry and missing-decision checks. No passing claim follows
from protocol schema or a mock environment update alone.

Stop before introducing a provider fork, tool proxy, general race framework,
new attempt object, or a second navigation implementation. If existing lifecycle
operations cannot establish the handoff, return the concrete limitation and
propose explicit recovery for failed decision/router turns. Do not silently
change that product contract.

Explicit recovery must itself establish a fresh tool environment and exact
ownership while preserving the conversation. Merely requiring `--retry` on the
same loaded engine does not repair the observed failure. Ordinary nondecision
retries need not be disabled by that narrower proposal.

## Experiment outcome and remaining decision

Supervisor inspected both additional native probes on 2026-09-28:

- `native-idle-resume-config-1` records a failed turn and thread status
  `systemError`. Unsubscribe succeeds. A new client resumes the same thread with
  generation 2 in its configuration, but the next tool still prints generation 1.
  The fresh-environment assertion fails. Despite the directory's name, the
  observed thread was not idle.
- `native-interrupt-resume-config-1` asks to interrupt that completed failed turn.
  Codex returns `no active turn to interrupt`; the probe stops there. It does not
  establish any later unsubscribe/resume result.

Both use real Codex0.157.1 with synthetic Responses in a private Home. These
operations do not repair the transport. They do not prove that every native
handoff is impossible or justify replacing a shared engine. Raw results and
scripts remain under `.lf/tmp/execution-model/`; logs under `.lf/tmp/cut-i/`.

The control `native-success-resume-config-1` passes: after a successful turn the
thread is `idle`, unsubscribe/resume retains the same thread and engine, and
the next tool prints generation 2. Supervisor inspected its results. This
narrows the demonstrated limit to the failed-thread path; it is not a passing
automatic-retry or shared-sibling proof. A provider change to reload eligibility
is a candidate explanation/repair, not an implemented Loopflow solution.

The bounded experiment is finished with a negative result. Keep the store owner
conversion and independent CI repair moving, but retain the valid automatic
retry as a failing integration requirement. Before expanding native machinery,
main must supply a concrete smallest proposal with its behavior, implementation
cost, and sibling/conversation preservation proof. Requiring explicit retry by
itself remains insufficient. Jack has not selected a retry-contract change.
This review selects no Flow edge, installation or installed-Home mutation;
ordinary checkpoint publication retains Jack's existing authorization.
