---
requires: a measured baseline, its limiter and any earlier trials
produces: the next hypothesis, or a decision that no clear way forward remains
action_style: procedural
---
Choose the next change worth trying, or say that none is.

Read the latest measurement, every row of the table in `scratch/measure.html`,
and the code behind the current limiter. A hypothesis names one mechanism, one
change and what it should do to the number: "defer X off the startup path, because it blocks first
paint, for about 20% off p50". "Try memoizing something" is not a hypothesis.

Prefer the cheapest idea that reaches the limiter: stop doing the work, avoid
repeating it, do less of it, do it later, then make it cheaper. Do not retry a
rejected idea unless something new suggests it will work. After several rejects in a row, change
category, combine near-misses or reread the source before concluding anything.

Write the hypothesis in the working design, so the next step can act on it.

When the caller gives you a decision protocol:

- **Iterate** with the next hypothesis when one has a credible path to moving
  the number.
- **Advance** when the request's target is met, or when no clear way forward
  remains. Running out of worthwhile ideas is how this finishes. Say what you
  ruled out and the best idea left.
- **Blocked** only when the harness or the workload has failed and no result can
  be trusted.
