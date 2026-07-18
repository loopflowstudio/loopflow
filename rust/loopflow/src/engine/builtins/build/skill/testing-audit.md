---
description: Audit whether a repository tests valuable behavior at the right rigor and cost, then improve its testing workflow, monitoring, and guidance.
requires: repository with tests or verification workflows
produces: scratch/testing-audit.md, testing infrastructure and guidance improvements
default_agent: codex
action_style: procedural
---
Audit the repository's testing system as an evidence portfolio, not a coverage
contest. Find what earns confidence, what tests implementation accidents, and
where agents or humans repeatedly pay for the same proof.

## Orientation

Read the repository guidance, test entrypoints, CI workflows, release gates,
demo/production checks, and any local run or trace ledger. Keep raw prompts,
commands, output, credentials, and customer data private; reports contain only
aggregates and named repository artifacts.

Write findings and the change design to `scratch/testing-audit.md`.

## Workflow

1. **Inventory the proof surfaces**
   - Count suites and tests by language/component.
   - Map local runners, CI jobs, release/host gates, production smoke/canaries,
     and demos.
   - Record wall time, parallelism, failure frequency, flake/retry evidence,
     and the slowest useful boundaries.

2. **Map behavior to proof**
   For each important user or operational behavior, name its cheapest credible
   proof: focused test, affected suite, integration boundary, real product
   path, production observation, or release gate. Mark unproved behavior and
   proofs with no behavior attached.

3. **Read representative tests**
   Sample hotspots, slow tests, mock-heavy tests, fixtures, workflow-string
   assertions, and cross-language contracts. Judge assertions by observable
   result. Mocks may block side effects; mock call wiring is not a result.

4. **Measure lifecycle cost**
   Attribute test/check intervals to skills or phases. Merge overlapping
   intervals before comparing them with agent wall time. Separate full/broad,
   focused/selected, and static/build work. Look for distinct commands that
   repeatedly recompile the same graph, not only exact retries.

5. **Delete and redesign**
   - Delete tests that only freeze prose, duplicated configuration, mock calls,
     or implementation structure unless they protect a concrete safety rule.
   - Replace broad inner-loop runs with focused behavior, affected-suite gates,
     and exact-tree evidence reuse.
   - Give implement, compress, lint, rebase, gate, CI/release, and demo distinct
     proof ownership so a phase transition does not trigger a redundant run.
   - Prefer real configured/deployed product proof when it is safe and
     observable. Never mutate production solely for an audit or demo.

6. **Change the system**
   Implement the highest-leverage safe improvements: runner selection/reuse,
   privacy-preserving timing, CI or package smoke, lifecycle skills, test
   deletion, and user guidance. Keep timeouts as hang guards, not performance
   verdicts.

7. **Verify the audit**
   Run focused tests for the infrastructure changed. Exercise timing/reporting
   against a safe real or copied ledger. Demo one real path. Review the diff for
   privacy, false confidence, and whether each new test asserts behavior.

## Output

`scratch/testing-audit.md` should contain:

- the proof map and important gaps;
- measured local and CI cost, with capture limitations;
- keep/delete/redesign findings with concrete examples;
- changes made and focused validation results;
- follow-up tasks only where more evidence is required.

Stop when the workflow is cheaper and more truthful, not when every test has
been cataloged.
