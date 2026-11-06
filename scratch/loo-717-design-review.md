# LOO-717 — Simulated design review

November 3, 2026. Jack Heart requested a simulated review from Mitchell Hashimoto.
This is an agent-authored application of themes in Mitchell's public writing,
not his review, quotations, endorsement or a new decision from Jack.

## Assessment

The chosen host boundary is sound. The next useful work is a small real execution
path and its failure cases, not more hypothetical lifecycle UI. Implementation
has not been authorized by this review request.

1. **Make Desktop an ordinary consumer.** lf owns Session execution and its idle
   ending. Desktop and Rex issue the same retention/release intent; they own
   selection, native layout and when their views stop retaining. Do not put
   Desktop tab rules in shared Session management, or copy Rex's split tree.
   This applies the reusable-core/native-consumer separation described in
   [Libghostty Is Coming](https://mitchellh.com/writing/libghostty-is-coming)
   and [About Ghostty](https://ghostty.org/docs/about).

2. **Prove what a retention request means when a client disappears.** A durable
   boolean can retain an agent forever after a crash. A socket disconnect can
   also mean only a lost viewer while a remote host still retains the workspace.
   Define the real owner, acquisition, release and recovery using existing
   attachment/process evidence. Do not invent a second layout registry or turn
   missing host observations into confirmed closure. This is the key unresolved
   engineering contract, not a question Jack needs to answer about tabs.

3. **Give the two cleanup stages independent, explicit conditions.** An agent
   ending at two idle minutes must not accidentally start another fifteen-minute
   timer. The uncomplicated completed-and-away case is two minutes plus thirteen.
   Agent activity, host retention and Task completion are separate facts. Write
   down how deadlines interact with ongoing work and renewed use; then test those
   boundaries with a controlled clock. The current prototype restarts a grace
   after work/retention holds end; that is a proposed detail, not Jack's explicit
   timing decision. Reuse LOO-709's active-work evidence. Shell-server output
   alone must not masquerade as an active agent turn indefinitely.

4. **Remove speculative post-cleanup restoration from the required scope.** Jack
   questioned what “return” even meant. An old Task's history can remain recorded
   without designing automatic checkout resurrection or a new continuation UI.
   Preserve ordinary conversation resume while its checkout still exists.
   A future concrete request can determine post-deletion continuation. This
   recommendation follows the narrow, usable slices advocated in
   [My Approach to Building Large Technical Projects](https://mitchellh.com/writing/building-large-technical-projects).
   It is a scope recommendation, not an approval to build.

5. **Honor the disposal policy Jack selected.** Eligible completed-Task cleanup
   removes uncommitted checkout files too. Do not quietly reintroduce dirty-file
   retention or another confirmation dialog. Still wait for requested active work,
   honor host retention, and operate only on the Task's recorded resources.
   Changed committed refs remain governed by the existing separate rule; no new
   decision about them was made. Keep the cleanup result honest when an operation
   fails halfway through.

6. **Review a working terminal experience.** First show a shell retaining its
   identity and jobs through release/reattach, and Close actually removing its
   owned processes. Exercise the remote path without any local-prompt fallback.
   Then demonstrate two-stage Task cleanup and the same retention contract through
   both hosts. Inspect actual process/PTY/socket leftovers. A simulation explains
   the policy but proves none of that. This applies the emphasis on frequent
   tangible demos in the projects essay, and on lived interaction in
   [You Have to Feel It](https://mitchellh.com/writing/feel-it).

Recommended next design artifact: one short state/ownership contract and one real
end-to-end demo plan. No additional broad product questionnaire is needed.

Check: source-grounded simulated review only; no runtime code changes or Graph launch.
