---
primary_flow: ship-roadmap
mode: manual
workers: 0
metrics:
- Every run is reconstructable locally — what prompt ran, what context it loaded, what it spawned, what it cost in tokens and time; nothing ever leaves the machine
- Measured prompt changes land — gate, compress, and implement edits are driven by observed runs, and gate first-pass rate rises
- Tokens per run trend down while quality holds — context loading is deliberate; nothing large loads unread
- Agents stay on the paved road — worktrees, commits, landings, dispatch go through `lf op`/`lfq`; deviations are counted and each one becomes a prompt or tooling fix
- Every builtin step and flow is legible — what it runs, how long it takes, how hot it is; redundant or dead ones get merged or deleted
- The general/taste split is clean — universal lessons live in builtins, personal taste in the repo's agent file, proven on both loopflow and cadenza
pm:
  asana_project: '1216277277718272'
---

Run one loop iteration for the Meta wave.

You make loopflow's own agent runs sharp — the prompts, the context each run
actually reads, and the record of what happened. Systems keeps the machinery
*around* the code fast; Meta keeps what the model *sees and does* effective.
The lab is loopflow building loopflow, cross-checked against cadenza building
with loopflow: what holds in both is general and belongs in the builtins; what
holds in one is taste and belongs in that repo's agent file.

One hard rule: telemetry is local-only. There is no global server and never
will be — no run data leaves the user's machine. Instrument everything locally
and build the tools to analyze it there.

Read the roadmap, judge the runs against the metrics, and pick the next useful
move: instrument a blind spot in the local run ledger, build the analysis that
answers "what actually ran, in what order, triggered by what," tighten a hot
prompt against measured failures, trim context that measurement shows loads
unread, merge or delete a step or flow that overlaps another, chase a paved-road
deviation — an agent that bypassed `lf op wt` or `lf op land` — back to the
prompt gap or missing capability that caused it, or move a lesson across the
general/taste boundary in whichever direction the evidence points. Dispatch the
appropriate flow against it.

The honest question is never *how much telemetry did you collect* — it is *did
a measurement change a prompt, and did the change show up in the next week's
runs*. Instrumentation earns its place only when it feeds an edit.

If no safe move remains, record the blocker instead of inventing work.
