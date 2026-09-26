# Task 3 — open Sessions

One Session opens directly; multiple
Sessions open the Task overview with a short list before Description. Each row
has a title, provider, state and last-message excerpt. The Session name completes the Wave / Task / Session breadcrumb; with multiple
Sessions it becomes the conversation selector. Selecting a Session drills down Wave → Task → Session. The named Task
ancestor returns to Overview; there is no parallel view-mode switch. Each
draft and exact selected conversation are retained. Description remains on Overview.
The human simplified away the separate Session-with-context depth.

New session sits beside the Task title, following the final human decision.
Names are editable in place through the final breadcrumb; the website retains
local names and drafts only. Initial skill/musical-animal names and agent naming
guidance are specified in the governing design, not implemented by this fixture.
All Session identities, transcripts, activity and execution are explicit study
fixtures; the Linear title and Description remain the captured text. The Release outcomes fixture belongs to Feature / implement, iteration 3;
Verification cases and Acceptance notes are independent Task conversations.
Membership is explicit fixture data, not inferred from provider or activity.
None of these conversations is a Flow review gate. No real Session is launched,
transferred, completed or attributed by this prototype.

Claude authored the bounded Session component through `lf -b -m claude`; parent
integrated fixtures, existing navigation/draft ownership and scenario shell.
Review removed a render-time selection write and duplicate issue link.
Headless browser check passes: one/multiple scenarios, Overview and Session, exact
selection, independent draft retention through switching/Overview, no page errors.
Inspected lf screenshot captures task3-one.png and task3-multiple.png. These are
website composition proofs, not native/provider evidence.

The human accepted this composition for implementation. Follow
[the governing design](../main-view-task.md); production contracts and native
proof remain the next work.

Final reference check: New session appears once beside the Task title in all
three scenarios. Inline rename updates the breadcrumb, Session list and pane;
Cancel/Escape preserve the old name and current location. Rename and breadcrumb
navigation preserve the original Session draft. Browser check passes with no
page errors. Generator recovery, agent guidance and persistent rename remain
implementation work rather than browser fixture behavior.
