# Wave memory — break-test

## Learnings

### `lf debug` headless with no input is a recurring probe (2026-04-23)

The break-test wave keeps invoking `lf debug` headlessly with no clipboard and no inline error. Observed on:

- branch `jack-heart.break-test.20260423_1312`
- branch `jack-heart.break-test.20260423_1451`
- branch `jack-heart.break-test.20260423_1604`

The step spec (`code/debug.md`) tells the agent to *ask* when clipboard is empty — but headless mode forbids questions. Correct response is documented in `scratch/questions.md`: write a "no input" note, exit cleanly, do **not** fabricate an error to debug.

If you see this again: don't re-investigate. Append the new branch timestamp to `scratch/questions.md`, update this memory line, commit, exit.

### Prior work already committed

Commit `d0794a70 lf commit: debug` is the artifact from the previous probe — it contains an earlier version of `scratch/questions.md`. No code changes were produced, because no error was presented.

## Patterns

- `scratch/questions.md` is the designated sink for "headless probe with no well-defined answer." Prefer reinforcing an existing entry over creating a new file.
- `scratch/*` is cleared by `lf op pr land`, so these notes only persist across break-test runs because break-test branches don't land.
