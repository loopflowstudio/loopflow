# Open questions — break-test

## debug invocation with no error input (recurring)

`lf debug` has now been invoked headlessly **three times** with no error input:

- 2026-04-23 on branch `jack-heart.break-test.20260423_1312`
- 2026-04-23 on branch `jack-heart.break-test.20260423_1451`
- 2026-04-23 on branch `jack-heart.break-test.20260423_1604` (current)

Each run: no clipboard content, no `-c` output in the prompt, no inline error, clean git tree.

The step (`code/debug.md`) says: *"If clipboard is empty or no -c flag, ask what error to debug."* Headless mode forbids questions — the two paths are incompatible.

### Assumption

This is an intentional break-test probe. Correct behavior in headless mode is to exit cleanly and surface the missing-input condition here rather than fabricate an error to debug.

### Recommendation

`code/debug.md` needs an explicit headless branch. Proposed shape:

```
If invoked headlessly with no clipboard and no inline error:
  - Write a short note to scratch/questions.md explaining that debug
    needs an error input (clipboard via -c, or a user-provided message).
  - Exit 0. Do not fabricate an error to debug.
```

The current wording conflates interactive and headless paths, which forces the agent to either (a) invent an error, (b) stall, or (c) freelance a reply like this one. A structured "no input" signal is the only well-defined answer.

### Signal strength

Three occurrences on fresh break-test branches within a single day. The probe is now reliable enough that the pattern is clear — the fix (an explicit headless branch in `code/debug.md`) should be scheduled rather than continuing to re-surface the same question each run.
