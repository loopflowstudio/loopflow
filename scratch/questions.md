# Assumptions — 2026-11-04

The Task brief supplies the accepted outcome.
Bound the demonstrated terminal store retry loop and preserve the existing input
retention contract. Do not infer permission to discard drafts after unexpected
path loss. Historical request attribution is unavailable after the owners exited;
record that gap instead of claiming a specific syspolicyd caller.

A bounded disposable SQLite probe showed a moved WAL database still accepts writes
through its retained connection. Path loss does not universally cause the input
write failure; the fault-injection test proves failure/recovery handling, not the
historical incident's cause. No file mover or requesting hook is attributed.

Demo fixture reclamation is unresolved between LOO-725 and LOO-722. Jack Heart
prefers stopping owners and reclaiming disposable demos over indefinite retention,
and also requires durable conversation history to survive. LOO-722's marker
authorizes deleting a root's store and drafts on the premise that evidence lives
elsewhere; `demo.md` keeps private provider history inside the root and writes no
marker. Choice: (a) separate durable evidence from disposable runtime, making
eligible roots available to LOO-722 without a competing cleaner, or (b) retain
the complete unmarked root until explicit teardown. Current state is (b), the
simpler safe choice; it does not establish acceptance of indefinite retention.
Demo/review still owns that judgment.
