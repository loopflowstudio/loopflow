Turn a design doc into working code.

The design doc lives under `.design/`. It's auto-included in the prompt as part of repo docs. The doc contains data structures, APIs, constraints, and a "done when" verification step.

## Approach

Start with the data structures. Get the core types right first—everything else builds on them.

Then implement APIs one at a time. Follow the signatures in the design doc. If something's underspecified, make a reasonable choice and move on.

Run the "done when" check from the design doc to verify the implementation works.

## Constraints

**Don't commit.** Leave that to the caller. Just write the code.

**Follow STYLE.md closely.** Read it before starting. Match the style and structure already in the codebase. When in doubt, look at how similar things are done nearby.

**Stay in scope.** Implement what the design doc describes, nothing more. Note anything that should be added, but don't build it.

**Leave the design doc.** Don't delete `.design/*.md`—`lf review` writes its assessment under `.design/`, and `lf pr land` removes the `.design/` contents.

**Add documentation per the style guide.** The best documentation is simple code—descriptive names, type hints, clear APIs. Skip obvious docstrings. If a module needs explanation, add a brief comment at the top of the file, not a separate doc. Update existing READMEs when user-facing behavior changes. Document new CLI commands or user-facing features in the appropriate README.

**Add tests that prove it works.** Test user behavior, not implementation details. A good test proves something users care about actually works. Keep tests short and focused. If a test requires elaborate mocking, it's testing the wrong thing—write something simpler or skip it.

## If something's wrong

If the design doc is unclear or seems wrong, ask before proceeding in interactive mode. In batch mode, make the simplest, clearest choice and move on. The code can be rewritten if a different choice is needed later.

If implementation reveals a flaw in the design, note it. The design doc was scaffolding—it's fine for reality to diverge from the plan.

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

