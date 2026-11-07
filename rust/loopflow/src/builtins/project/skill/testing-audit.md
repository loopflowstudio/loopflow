---
description: Audit whether a repository's tests catch what people would notice breaking, at a fair cost, then fix its test workflow and guidance.
requires: repository with tests or verification workflows
produces: scratch/testing-audit.md, testing infrastructure and guidance improvements
action_style: procedural
---
Make the repository's tests catch what people would notice breaking, and stop
paying for the ones that don't. Coverage numbers are not the goal.

Read the repository guidance, test entry points, CI workflows, release gates and
any local run or timing log. Keep raw prompts, commands, output, credentials and
customer data out of the report; it holds totals and named files only. Write
findings and the planned changes to `scratch/testing-audit.md`.

## Workflow

1. **Inventory the checks.** Count suites and tests per language and component.
   List local runners, CI jobs, release gates, production smoke checks and demos.
   Note wall time, parallelism, failure rate, flakes and retries, and the
   slowest suites worth keeping.

2. **Map behavior to tests.** For each behavior people rely on, name the
   cheapest check that would catch it breaking: a focused test, an affected
   suite, an integration test, a run through the real product, a production
   check, or a release gate. List behaviors nothing catches, and tests that
   catch nothing anyone would notice.

3. **Read representative tests.** Sample hotspots, slow tests, mock-heavy tests,
   fixtures, tests that assert on prompt or workflow text, and cross-language
   wire tests. A good test asserts on a result someone would see. Mocks may
   block side effects; checking that a mock was called is not a result.

4. **Measure what it costs.** Attribute test and check time to skills or phases.
   Merge overlapping intervals before comparing with agent wall time. Separate
   full runs, focused runs, and build or lint time. Look for different commands
   that keep recompiling the same thing, not just exact retries.

5. **Delete and redesign.**
   - Delete tests that only freeze prose, duplicate config, check mock calls or
     pin implementation structure, unless they guard a real safety rule.
   - Replace broad inner-loop runs with focused tests, affected-suite gates, and
     reuse of passing results for an unchanged tree.
   - Give implement, compress, sync, gate, CI/release and realign separate jobs
     so moving to the next phase doesn't rerun the same suite.
   - Prefer checking the real configured or deployed product when that is safe.
     Never change production just for an audit.

6. **Change the system.** Make the highest-leverage safe improvements: runner
   selection and reuse, timing that keeps data private, CI or package smoke
   tests, lifecycle skills, test deletion and guidance. Timeouts catch hangs;
   they don't judge speed.

7. **Check your changes.** Run focused tests for what you changed. Run the
   timing or reporting against a safe real or copied log. Run one real path
   end to end. Reread the diff for privacy leaks, false confidence, and new
   tests that don't assert on behavior.

## Output

`scratch/testing-audit.md` holds:

- behaviors and the tests that catch them, plus the important gaps;
- measured local and CI cost, and what the measurements missed;
- keep, delete and redesign findings with concrete examples;
- changes made and the focused test results;
- follow-up Tasks only where a question needs more runs to answer.

Stop when the tests are cheaper and catch more of what matters, not when every
test is cataloged.
