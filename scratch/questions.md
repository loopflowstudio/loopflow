# Open questions

- Choose execution is still deterministic (alphabetical option) and ignores the choose prompt. Should we wire choose to agent invocation and parse a structured choice output?
- AgentStepRunner only uses config.area (no flow run area). Do we want to thread FlowRun.area into context assembly?
- LoopUntilEmpty now checks roadmap/<wave> and runs only Step items. Should loops support fork/choose items and propagate interactive steps instead of failing?
- Context assembly parity gaps remain (summaries, loopflow doc embedding, area parent docs, exclude patterns). Which pieces are required for Stage 2 parity?
