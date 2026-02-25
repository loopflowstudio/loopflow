# Open questions

- Runtime convergence design spans provider harness extraction, session/runtime adapter unification, and prompt convergence. This implementation only lands shared prompt prep + call-site wiring; should the next implement step prioritize harness extraction in `engine` or wave session routing first?
- Wave execution policy in the design calls for interactive steps to run via sessions. Current wave executor still pauses on `FlowAction::WaitInteractive`; no session-backed auto-launch path was implemented in this pass.
