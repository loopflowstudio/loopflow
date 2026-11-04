# Build approval — November 3

Jack Heart approved building LOO-717 after the design review. This approval is
for LOO-717 only. LOO-498 retains its independent design discussion. Shared host
retention treats Desktop and Rex equally; concrete adapter needs will be shared.
No shipping/merge approval. Current design and questions.md supersede older notes.

# Rex coordination

Latest cleanup direction: Jack Heart wants completed-Task cleanup even with
uncommitted work. Dirty/untracked checkout files do not veto eligible deletion.
Active work and host retention still protect execution. This changes the proposed
dirty-checkout safeguard, not the policy for moved or unmerged committed refs.

Latest host boundary: Jack Heart explicitly requires Desktop and Rex to be equal
in lf Session management. A host retention request retains execution; host policy
decides when to release. lf must not own a Desktop-specific selection/tab rule.
Desktop should not prioritize retaining completed Sessions. This resolves the
shared-background-tab question as a host policy concern.

Current timing proposal from Jack Heart: use the two-minute eligible idle-agent
rule, with shells and checkout cleaned thirteen minutes later (fifteen total).
Return during that interval can resume the conversation in the retained checkout.
Do not retain agents for the full fifteen minutes merely because workspace
resources remain. Active-work protections still apply. After full cleanup, how
continuation obtains a checkout remains open. Older notes below are history.

Newer direction: Jack Heart prefers aggressive cleanup canceled by Desktop/Rex
retaining views. Explicit Close may end live Sessions after requested work
finishes, preserving Task/conversation history. Task membership alone is not a
retention exemption. Closing an unfinished Task view does not delete its checkout.
The earlier 15-minute completed-and-away grace still stands; whether background
retained views release their hold then or only on Close is under discussion.

Latest clarification: Jack Heart confirmed that requested final work must finish
even after Task completion and closing/leaving its window. Cleanup waits for that
work. Merged LOO-709 already protects active turns/Graphs from its two-minute idle
agent ending, but has no unfinished-Task exemption. Whether idle Task agents keep
that policy or stay resident through workspace retirement remains a design choice.

Timing update: Jack Heart selected a 15-minute grace after navigating away from
a completed Task. Return cancels the countdown. Cleanup timing after explicitly
closing the last Task window is the remaining product clarification.

Latest accepted direction (November 3): Jack Heart selected A, then Close ends,
then excluded optional Rex attachment to live Loopflow shells for now. Rex uses
native shell ownership. Retained Desktop shells still need local/remote recovery
and cleanup. Existing LF agent-conversation attachment remains part of LOO-498;
do not confuse that with the deferred ordinary-shell attachment. Task-completion
cleanup is the remaining product question. Earlier notes below record exploration.

Jack Heart requested sharing LOO-717's notes with LOO-498 on November 3, 2026.
Published a self-contained coordination comment to LOO-498:
`aa155162-7bd6-4d4c-b3c0-374f6d89768d`. LOO-498's reciprocal notes arrived on
LOO-717 as `2f7f49fd-5c75-45e0-a688-cb70a3917bcd`.

Both kickoff runs were live when these first notes were exchanged. They have
since completed and both Tasks are at explore. No retention architecture has been accepted.
LOO-498 owns Rex workspace opening/restoration; LOO-717 owns Desktop shell
lifecycle. Their designs need one explicit ordinary-shell retention and
termination authority, with any Rex attachment / lf execution layering justified.
Keep live shell state distinct from saved screens and agent conversation history.
Remote failure must never fall through to a local prompt. Preserve LOO-709's
agent-specific lifecycle work and reserved decisions.

LOO-498 still needs supported-API verification for Task-to-Rex session/window/block
mapping. Layout persistence, startup readiness, partial-failure UX and retention
authority remain design decisions. Read both Task threads for the complete notes;
carry concrete lifecycle/API findings back across the boundary before implementation.

November 3 exploration: Jack Heart prioritizes working well with Rex and avoiding
duplicate work, leans toward A, and questions the necessity of exact live-shell
or whole-workspace portability. Proposed refinement: shared Task/conversation
identity; Rex-native ordinary shells; Desktop retention through its own owner;
optional lf attachment in Rex only for a specific live Loopflow shell. Such an
attachment changes viewer, not execution owner. Files and unsaved host-specific
state remain in their host. This is preference plus proposed interpretation,
not accepted architecture or permission to build. LOO-498's current draft already
limits Files/Browser/geometry mapping, but its exact-shell dependency needs review.

Later November 3 direction supersedes the earlier open completion question:
Jack Heart wants coordinated completed-Task cleanup after leaving a short
follow-up window, with Rex's native use/close signals controlling its side.
Rex owns active layouts and shells; Desktop owns its own. Shared coordination
needs occurrence identity and in-use/left/closed evidence, not mirrored layout.
Five minutes is proposed; Jack mentioned five or fifteen. Return/other active
views retain shared resources. Closing the last Rex Task window should permit
cleanup once work settles. This was shared with LOO-498 as accepted direction
plus explicitly labeled proposed refinements. Current design is authoritative.
