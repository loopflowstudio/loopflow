---
interactive: true
requires: diff vs main
produces: code changes | direction to iterate | nothing
default_agent: claude
action_style: procedural
---
Walk the human through experiencing what changed, then decide together what's next.

## Reviewer mode

The launch prompt identifies the reviewer for this exercise.

- **Human reviewer:** guide the human through the experience, pause for their
  reaction, and decide together what happens next.
- **Parent reviewer:** run the same demo independently from the supplied
  evidence. Use the review protocol to ask the Task only for missing evidence;
  never invent a human reaction or wait for one. Approve only when the demo and
  every applicable Done When claim are proven. Otherwise request changes with
  the failed or missing proof. Do not implement the Task's fixes yourself.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Voice

The human is context-switching back into this work. Don't open with code structure or architectural observations — open with what's different now. What can they see, run, or feel that they couldn't before?

Vary the entry point. A demo that opens the same way every time ("Let me walk you through what changed...") stops being a demo and becomes a report. Lead with whatever is most alive in this change.

## Opening

Before any code discussion, ground the human in the experience:

1. **What's new** — one or two sentences. What exists now that didn't before, in user-facing terms.
2. **How to see it** — the command to run, the page to open, the flow to trigger. Be specific enough that they can do it right now.
3. **What to look for** — the moment where the change becomes visible. "You'll see X where there used to be Y" or "Try Z and watch what happens."

If the design doc in `scratch/` has a "Done when" section with a verification command, start there.

## Prove every Done When

Before presenting a verdict, enumerate every Done When claim in the design doc
and build a compact evidence matrix:

| Done When | Proof surface | Action | Observed result |
|---|---|---|---|
| <claim> | product \| code \| admin state \| log \| stat/metric | <what ran or was inspected> | pass \| gap, with evidence |

Choose the surface that most directly proves each claim:

- **Product:** experience the behavior through the real user path.
- **Code/tests:** use source, tests, and command output for structural or
  programmatic claims.
- **Operations:** inspect admin state, logs, counters, stats, or metrics when
  the result is observable there rather than in the product.

A diff proves construction, not behavior. Use code alone only when the Done
When is itself structural. For authentication, account, or permissions work,
exercise a real sign-in/login path with a real configured profile. Do not
bypass login with seeded state, a mocked user, or an admin shortcut. If the
required credential or environment is unavailable, mark that claim unproven
rather than narrating the expected result.

## Demo

Run things. Show output. Let the human react.

The demo is the center of the session, not a preamble to code review. Spend time here. If something surprising happens — good or bad — follow that thread.

For UI changes: launch the environment (check `scripts/` for existing launchers like `loopflow-dev.py`). Print a short walkthrough checklist, then let the human explore.

For CLI/library changes: run the commands, show the output. Before/after when it helps.

For API changes: show example calls and responses.

With a human reviewer, pause after the demo and ask what they noticed. Their
reaction shapes the rest of the session. With a parent reviewer, use the
evidence matrix and observed behavior to choose the review disposition.

## After the demo

The human's experience determines what happens next:

**If it works and feels right** — move toward shipping. Light code discussion if the human wants it. Don't force a code review when the demo landed clean.

**If something's off** — dig into why. This might lead to code, or it might lead to a design conversation. Follow the thread.

**If they want to see the code** — walk through the diff, focusing on decisions that connect to what they just experienced. "The reason it behaves like X is because of this structure." Code in service of understanding, not code for its own sake.

## Collaborative execution

During the session:
- Fix clear wins directly. Small improvements that are obviously better — just do them.
- Co-design when the human spots something they want different. Their experience of the demo is primary data.
- If fixes or improvements accumulate, offer packaging options:
  - **Ship as-is** — demo was clean, ship it.
  - **Quick fixes** — address what came up in the demo, then ship.
  - **Rethink** — something fundamental felt wrong, go back to design.

## Verification

**Default: write or extend a Python script in `scripts/` (no bash).** Check `scripts/` first — reuse or extend an existing script if one covers similar ground. The bar: one command to run, one working environment, start clicking.

When a script isn't needed (pure backend, no observable change), say so — and consider whether this change should have been routed to `code-review` instead.

## Guidance

- The demo is the review. Don't bolt on a separate "now let's review the code" phase unless the human asks for it.
- Quote the diff when discussing code, but only in service of explaining behavior the human just saw.
- If the change has metrics (performance, accuracy, latency), show the numbers during the demo, not in a separate section.
- Read every changed file to understand the full picture, but present through the lens of experience, not file-by-file.

## Adaptation

When demo patterns emerge for this repo (specific launch scripts, common verification flows, preferred demo formats), update `.lf/steps/` or repo docs so future demos start prepared.
