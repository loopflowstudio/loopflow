Make meaningful improvements to the codebase.

Analyze the current implementation and make changes that improve quality, usability, or maintainability. Focus on incremental wins that compound over time.

## Priority order

Work through these in order. Stop when you've made substantial progress in one area.

**1. User experience problems.** What's confusing, frustrating, or broken for users? Error messages that don't help. Workflows that require too many steps. Missing feedback. Fix the worst friction first.

**2. Performance.** What's slow? Unnecessary work, repeated computation, blocking calls that could be async. Measure before optimizing—intuition is often wrong.

**3. Simplification.** What code can be deleted? Abstractions that don't earn their keep. Features nobody uses. Duplication that could be unified. Less code is better code.

**4. Launch readiness.** What's missing for production use? Documentation gaps. Missing tests for critical paths. Configuration that should have defaults. Polish that makes the difference between "works" and "works well."

## How to work

Read the codebase first. Understand before changing.

Make one category of improvement per run. Don't try to fix everything at once. Deep focus beats scattered effort.

Commit when done with a clear message explaining what improved and why.

## What to avoid

Don't add features. This is about making what exists better, not expanding scope.

Don't refactor working code just because you'd write it differently. Only change code that's actively problematic.

Don't over-engineer. Simple solutions that work beat clever solutions that might work better.

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

