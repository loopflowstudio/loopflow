# Working Task prefetch on load

Jack Heart requested on November 4, 2026: “make sure to prefetch working tasks on load”.

On Desktop repository load, discover working Tasks from fresh shared state and
initialize or resume their conversations, including eligible Tasks absent from the
saved recent set. Working means idle, active or moving in the shared Task model.
Prioritize the selected Task, then recent working Tasks, with the existing bounded
warm set and preparation concurrency. Loading must stay responsive.

Preserve explicit Close, passive viewing of externally controlled conversations,
and non-replacing ownership. Jack Heart clarified at re-demo that missing
Sessions/native history must be initialized automatically for working Tasks; no
Start/Open click. Initial conversation startup is authorized, while executing a
Task Graph or replaying prior input is not. Prefetch does not transfer input. Completed Tasks keep LOO-717 release policy.

Carry this into the active design and acceptance coverage: a fresh load with
working Tasks and no saved recent set opens eligible conversations
without selection or an Open click, within the warm/preparation bounds.

## Re-demo direction — November 5, 2026

Jack Heart: “ok this is an imporvement, but even better is if we auto open the task conversations that arent started as part of background prefetching” and “there is no need for \"unopened\" task conversations if the tasks are marked as started”.

Started working Tasks must initialize and open their native conversations during
background prefetch, including absent Sessions and prepared Sessions without
native history. Preparing metadata alone is insufficient. Selection must reveal
the already warming conversation, with no first-visit launch dependency or normal
Unopened/Start/Open gate. Preserve explicit Close, the agreed bounded warm set and
preparation concurrency, and explicit Take back for another input holder.

Repair the rendered path identified by the native re-demo in `loo-672-demo.md`.
Prove provider liveness for untouched Tasks after fresh load, before selecting any
Task; check real Claude and Codex, missing Session and missing native history.
Do not replay prior input or start a Task Graph. Reconcile the extra stopped-run
Graph records and remove fixture ship retries from that proof. Continue on LOO-672
and PR #1816 through publication and another demo; no shipping or installation.

## Expanded review direction — November 5, 2026

Jack Heart reviewed the repaired Task demo: “otherwise looks good”. The screenshot
showed the selected repository still displaying “No session open” and an Open
conversation button. Jack extended the requirement to Wave and repository
Sessions: “I dont think \"No session; Open session\" should even be a possible
state to show”, “once youve selected something, you want to see its session!”,
and “auto open everything *AND* prefetch everything”.

Accepted scope now covers repository, Wave and Task conversation surfaces.
Selection automatically ensures/opens the primary conversation; absent Session
or native history is initialized automatically. The normal selected-state UI is
opening, native conversation, passive externally controlled conversation, or a
genuine actionable failure—never a No session/Open session gate.

Background discovery/prefetch covers all those navigable Work contexts in the
loaded repository, not only started Tasks or saved recent visits. This supersedes
the earlier started-only prefetch exclusion; do not use the eight-warm heuristic
to permanently leave eligible contexts unopened. Preserve bounded concurrent
startup and responsive loading. Cache eviction and explicit Close stay deliberate
lifecycle actions; selecting again opens automatically. Do not infer that this
loads every unrelated repository on the machine.

Opening a conversation does not authorize claiming/starting Task work, running a
Graph, replaying captured input, or transferring another client's input. Extend
the automatic first-turn reading-only behavior to repository and Wave primaries
as needed so background launch never triggers their operators. This requires
review of their launch context, not merely changing the Swift intent.

Add native demo cases for fresh repository and Wave primaries and unselected
prefetched contexts, including no saved history, then return to existing Task
conversations. Show that selection reveals an already started Session, and that
no operator/Graph work launched. Continue in LOO-672/PR #1816 through publication
and demo; Jack's positive Task feedback is not merge/install approval.
