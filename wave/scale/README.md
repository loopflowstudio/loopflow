# Scale

## Vision

Loopflow manages an org's work, not just one task. Cross-repo portfolios, wave coordination, multi-flow execution. The system that makes many agents work together.

### Not here

- Single-wave execution improvements (that's engine/craft work)
- Remote connectivity (that's foundation/trust)
- UI for scale features (Concerto sprints live here because they're vertical — you can't build chord UI without understanding FlowRun)

## Strategy

Build the execution model first (FlowRun container), then cross-repo primitives (commits, stimulus), then surfaces (chord UI, portfolio UI). Each layer teaches what the next layer needs.

## Goals

- WaveRun/FlowRun split: iterations own branches and PRs, flow executions happen within them
- Reactive stimuli create triggered FlowRuns, not new WaveRuns
- Cross-repo work is ergonomic — repos resolve by name, commits split automatically
- Chords group waves visually and enable inter-wave listening
- Portfolio view shows the DAG of related repos

## Risks

- **Listen fan-out.** Many waves listening to one source triggers N runs simultaneously. No concurrency limiting today.
- **Cross-repo commits have no rollback.** A failure in one repo is reported, not compensated.
- **FlowRun executor refactor is the riskiest change.** The executor loop fundamentally changes from operating on WaveRun fields to FlowRun fields.
- **Branch sub-flow items silently skipped.** The branch executor only handles Step items.

## Metrics

- Listen stimulus latency: seconds from source wave completion to triggered run start (target: <5s)
- Cross-repo context resolution latency (target: <2s)
- Number of orphaned FlowRuns per week (target: 0)
- Single-repo test suite pass rate before and after cross-repo changes (target: 0 regressions)
