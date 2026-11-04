# Working Task prefetch on load

Jack Heart requested on November 4, 2026: “make sure to prefetch working tasks on load”.

On Desktop repository load, discover working Tasks from fresh shared state and
prefetch their existing conversations, including eligible Tasks absent from the
saved recent set. Working means idle, active or moving in the shared Task model.
Prioritize the selected Task, then recent working Tasks, with the existing bounded
warm set and preparation concurrency. Loading must stay responsive.

Preserve explicit Close, passive viewing of externally controlled conversations,
and explicit Start for missing native history. Prefetch starts no Graph or new
agent turn and does not transfer input. Completed Tasks keep LOO-717 release policy.

Carry this into the active design and acceptance coverage: a fresh load with
working Tasks and no saved recent set prefetches eligible existing conversations
without selection or an Open click, within the warm/preparation bounds.
