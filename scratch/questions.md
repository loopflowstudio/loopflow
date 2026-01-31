# Open Questions

- Choose execution is still deterministic (alphabetical option) and ignores the choose prompt. Should we wire choose to agent invocation and parse a structured choice output?
- AgentStepRunner only uses config.area (no flow run area). Do we want to thread FlowRun.area into context assembly?
- LoopUntilEmpty now checks roadmap/<wave> and runs only Step items. Should loops support fork/choose items and propagate interactive steps instead of failing?
- Context assembly parity gaps remain (summaries, loopflow doc embedding, area parent docs, exclude patterns). Which pieces are required for Stage 2 parity?

## Improvise UX

### Prompt field not wired through

The `StepRunner` has a prompt text field that accepts user input, but `launchInteractiveSession()` and `runWave()` don't accept a prompt parameter. The infrastructure to pass ad-hoc prompts to steps isn't implemented.

**Options:**
1. Add `prompt: String?` parameter to `launchInteractiveSession` and pass through to the terminal command
2. Remove the prompt field (breaks design intent)
3. Leave as-is, document as future work

This matches the existing FlowPicker behavior—prompt fields exist but don't actually pass through.
