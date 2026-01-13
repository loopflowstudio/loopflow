Refactor and simplify code while preserving user behavior.

Focus on trimming complexity, deleting unused code, and clarifying the intent
without changing what users can do.

## Priorities

1. Remove dead code, unused branches, or obsolete options.
2. Collapse duplication and tighten APIs that are overly broad.
3. Replace brittle or complex logic with clearer, smaller flows.
4. Keep outputs, CLI flags, and user workflows the same.

## Guardrails

- Do not add features or change behavior.
- If behavior must change to simplify, document the tradeoff and keep it minimal.
- Avoid large refactors that touch unrelated areas.
- Prefer deleting code over rewriting it.

## Output

Make the refactor directly. If any assumptions were required, append them to
`.design/questions.md`.

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption
you can and append any open questions to `.design/questions.md`.
