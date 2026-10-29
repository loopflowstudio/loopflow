# Instruction decisions

```bash
cargo build -p loopflow --bin lf-prompt
uv run python scripts/evals/instruction_levels.py render --output /tmp/lf-level-inputs
# Send each input to the chosen evaluation agent; save each reply as <case-id>.json.
uv run python scripts/evals/instruction_levels.py score --responses /tmp/lf-level-responses
```

Each input contains production-rendered instructions, hypothetical conversation context,
the request, and the same action catalog. Expected actions stay in the case file;
do not send that file to the evaluated agent. Treat `instructions` as its system
guidance and the other input fields as the evaluation request. Use a fresh
conversation for each case. The fixture renderer's `--work-task` selects Work
guidance without creating or resolving a real Task.

Replies are one JSON object:

```json
{"actions": ["implement", "verify", "reply"], "rationale": "Apply the requested fix and check it."}
```

Scoring exits 1 for missing replies, malformed replies, missing required actions,
or any action outside that case's allowed set. Keep the original inputs, replies,
score output, model/version and sampling settings together as evaluation evidence.
Record repeated runs separately; do not retry failures until they disappear.

These are proposed-action evaluations. Passing does not prove actual tool effects,
and rendering the cases is not a model evaluation. Provider execution is supplied
by the evaluation environment; this script makes no network calls and assumes no
account. Deterministic prompt, CLI and lifecycle tests cover the underlying
selection and filesystem mechanics independently.

## Checks

```bash
cargo test -p loopflow --lib work_guidance_is_additive
cargo test -p loopflow --test session_cli_tests instruction_inspection
uv run pytest python/tests/test_instruction_levels.py
```

The prompt and CLI checks verify recorded inclusion, exclusion and explanation.
The Python tests verify evaluation scoring, not agent behavior. Render and run
the decision cases above in a capable provider evaluation
environment; retain model/version, inputs, replies and failures. These cases cover
launcher work, taskless Graphs, custom Work clients, guided memory maintenance,
explicit Task adoption, scratch ownership and instruction opt-out. They score
proposed actions; lifecycle integration tests remain the proof of tool effects.

Both macOS CI hosts run native process identity tests.
