# Work API and execution lifetime

## Empirical provider-mode comparison

Jack Heart requested independent background research and experiments comparing
stream-json with native Claude execution without a viewer, using historical
rationales and current LOO-415/LOO-416 goals. A necessary restart on explicit
resume remains allowed, but uninterrupted native reattachment is preferable.
The stream-json recommendation is provisional, not a decision to preserve it
regardless of evidence. Compare real native TUI execution with structured print
mode; “headless” alone does not distinguish them. The independent report belongs
in `scratch/claude-execution-tradeoffs.md`; the existing implementation Flow
continues to own production changes.

## Accepted native Claude requirement

On October 16, Jack Heart answered “Native Claude UI is required” when asked what
`lf resume` must show for a still-running headless Claude Session. Live output
plus lf input/control is insufficient.

Jack Heart immediately clarified: “resume can restart te process if necessary
thoug”. This supersedes the earlier interpretation that every persistent Claude
execution must have a native terminal from launch. Explicit `lf resume` may
restart the provider when necessary to present the native UI while preserving
the durable Session and saved native conversation. Prefer live attach where it
works. A necessary restart is recovery/transition, not a claim that the same
in-flight turn or pending request survived. Do not automatically resubmit input.

Retain structured headless execution where appropriate. No custom lf substitute
UI or universal conversion of headless Claude launches to native PTYs is required.
Preserve headless/Flow completion evidence, cancellation and account/context
settings across the explicit transition. Viewer or creator disappearance alone
must never stop, restart or replay execution. Attach-only operations remain
attach-only; the explicit resume command has the necessary-restart permission.
Claude's background supervisor is not selected; its lifecycle conflicts remain
recorded below.

Stable capture/provider ownership and separate viewer input fencing are necessary
implementation work within the already accepted owner direction. Revise their
record/transaction shape as needed in this Task, retaining existing data ownership
and the one-draft migration rule. The counterexamples in the handoff establish
what must be removed; they do not require fresh approval for those internal repairs.

## Latest clarification

Jack Heart clarified that `lf resume` is an API without an explicit Work-model
scope that can nevertheless resolve a persistent Session. The earlier command
category split below is therefore incomplete: lifetime follows the resolved
Session, not whether the public command spells Task or Wave. Any entry point
opening that Session must use its independent owner. Foreground viewing versus
returning immediately remains a separate caller choice. Shared lifetime behavior
belongs below command routing at Session launch/connect. This clarification
does not require every contextual command to return immediately.

## Native attachment evidence, October 16

Installed Claude Code 2.1.302 help advertises `--bg` and `claude attach`;
OpenCode help advertises `attach <url> --session <id>`. These are capabilities
to evaluate, not evidence of Loopflow continuity.

[Claude's official documentation](https://code.claude.com/docs/en/agent-view)
explicitly rejects combining `--bg` with `--print`: its background attachment
does not open the current stream-json adapter. It uses a shared supervisor,
can restart unexpectedly exited workers, and stops unattached idle workers.
Moving an existing foreground session to background can interrupt in-flight
work. Do not adopt it as proof of the accepted no-replay owner contract.

[OpenCode's official CLI documentation](https://opencode.ai/docs/cli/#attach)
supports attaching the native TUI to an existing `serve` backend and selecting
the existing session. That is a concrete candidate for the current HTTP/SSE
adapter, subject to same-process/request, input-fencing and zero-viewer tests.
The claim that all structured Claude/OpenCode runs require a new lf UI is too
broad. Jack Heart's later necessary-restart allowance resolves the Claude
print-mode product boundary without a custom lf UI.

## Earlier discussion

October 16, 2026. Jack Heart clarified that the motivation for LOO-415 is enabling
LOO-416, then proposed background handoff as the distinction between Work-model
APIs and bare lf APIs: “i guess basically any *work model API* wants to do that,
vs *bare lf APIs*”.

The conversation's proposed interpretation is scoped to APIs that start execution:
Work-model commands resolve the workspace/Workflow, hand off durable execution and
return its identity, with waiting an explicit choice. Bare skill/Flow invocations
retain ordinary foreground streaming and exit-status behavior. Both use the same
independent execution owner, so returning immediately and surviving caller loss
are separate properties. Work APIs retain their domain rules, including workflow
edge selection and result recording, without a second background execution path.

This discussion does not waive existing acceptance or select new CLI flags/defaults.
LOO-415 supplies the handoff and attachment primitives; LOO-416 retains the lifecycle
and caller integration scope recorded in [the handoff](loo-415-transport-handoff.md).
Read/status/edit operations do not start execution merely because they address Work.

Jack Heart specifically identified `lf resume` as a concern. This qualifies the
background-by-Work interpretation: contextual selection and foreground attachment
are independent of execution lifetime. Proposed behavior: `lf resume` selects the
existing conversation and remains its foreground viewer; live execution attaches
without stopping, replacing, or replaying it. Confirmed stopped execution can
resume native history under the independent owner. Unknown liveness must not start
a duplicate. At baseline `72dbe0344f`, Resume shared the connect path, including
Claude/OpenCode's `stop_headless_for_resume`. The working implementation now
separates retained terminal attachment from explicit native transition, as
recorded in the handoff plan.
Cover this actual public entry point in transport acceptance, alongside explicit
Session attachment. This is not authorization to resume a Task workflow edge.
