# Research: harness prompting

Research snapshot: 2026-07-16. This compares Schema Harness's published ARC-AGI-3
system and traces with OpenAI's published Cycle Double Cover prompt, then maps
the transferable mechanisms onto Loopflow's prompt stack.

## System understanding

Loopflow has three prompt layers already:

- `PROMPTS.md` is author-time doctrine.
- `rust/loopflow/src/engine/builtins/LOOPFLOW.md` is the universal operating
  document injected once into normal prompt assembly.
- Builtin skills carry methods specific to research, debugging, QA, design, and
  coordination.

That separation is the right place to integrate these techniques. Copying the
same long method into every skill would increase context while making local
prompts less decisive.

### Schema Harness

Schema's outer loop is `observe -> deliberate -> execute -> record`. Inside
deliberation the agent edits an explicit world program, replays it against the
complete append-only transition history, searches the certified program, and
commits an action queue through one tool. Each real transition is compared with
the model's prediction. One mismatch drops the remaining queue and returns the
agent to deliberation.

The initial turn prompt is unusually operational. It presents current state,
legal actions, model/history status, writable and read-only boundaries, a
living `notes.md` schema, the raw observation, and exactly how the turn must
end. The notes separate guessed action semantics, current-level interpretation,
hypotheses, and confirmed cross-level facts.

The site reports 98.98% RHAE for its retained Claude Opus/Fable pairing and a
42.83% Claude Code scratch-snapshot baseline with the same model pairing. The
results are self-reported on the public set, not a held-out or independently
verified evaluation. The released traces do provide inspectable event logs,
session artifacts, notes, model snapshots, and a score recomputation utility.

### Cycle Double Cover prompt

The two-page CDC prompt uses a different mechanism. It first defines the target
with mathematical precision, restates the exact generality required, and names
many results that do not count. It then governs a large search portfolio:

- diverse approach families rather than fixed assignments;
- early independence from the favored route;
- an explicit family registry;
- blocked-route discipline for theorem-strength gaps;
- delayed cross-pollination;
- adversarial audit against a named edge-case list;
- concrete lemmas, equations, constructions, or counterexamples rather than
  status reports;
- repeated synthesis and dynamic reallocation by the root agent.

The document says this prompt led to a proof, but it is one successful prompt,
not an ablation. It does not isolate which instruction caused the result.

## Transfer map

| Source mechanism | Loopflow translation |
| --- | --- |
| Executable world model | Externalize the cheapest useful model: design, invariants, causal chain, test, prototype, or simulator |
| State grounding plus mechanism discovery | Reconsider the representation or boundary when local rule patches keep failing |
| Append-only transition timeline | Preserve logs, fixtures, reproductions, receipts, and observed facts separately from editable hypotheses |
| Full-history backtest | Validate against all relevant old and new evidence, not only the latest green case |
| Discriminating environment action | Prefer the smallest safe check whose outcomes separate leading explanations |
| `commit_actions` boundary | Stage and validate before external or irreversible side effects |
| Abort queue on misprediction | Stop dependent steps on surprising output; update the model or plan first |
| Persistent, pruned `notes.md` | Keep concise durable scratch state outside the provider transcript; prune stale guesses |
| Exact theorem statement | Define observable success, near-misses that do not count, edge cases, exclusions, and proof |
| Approach-family registry | Track mechanisms, evidence, exact gaps, and status when parallel search is authorized |
| Block theorem-strength gaps | Do not call a reduction progress when it merely relocates the original uncertainty |
| Adversarial proof agents | Derive QA cases from the exact contract and attack the survivor, not a generic checklist |

## Tensions

- **Prompt versus enforcement:** Schema's strongest guarantees come from tools:
  append-only history, complete replay, and a single action boundary. Prose can
  request the same discipline but cannot guarantee it.
- **Universality versus prompt tax:** evidence discipline applies broadly;
  executable simulators, BFS, large fan-out, and eight-hour minimums do not.
- **Persistence versus truth:** forcing continued search can prevent premature
  surrender, but “assume a solution exists” can also encourage fabrication if
  the prompt weakens verification.
- **Independence versus reuse:** early cross-pollination creates convergence;
  permanent isolation wastes discoveries. The handoff should happen only after
  each approach exposes its real mechanism and gap.

## Recommendations

### Put a short evidence loop in the universal operating prompt

**Observation:** Every run benefits from an explicit finish line, durable
evidence, discriminating checks, and replan-on-counterexample.
**Cost:** A small token charge on normal runs.
**Benefit:** The method reaches all standard prompt assembly without cloning it
into every skill.
**Verdict:** Worth it.

### Put specialized doctrine only in skills that exercise it

**Observation:** Research, debugging, QA, kickoff, and Project pursuit have
different uses for the same mechanisms.
**Cost:** Several focused prompt edits and tests.
**Benefit:** Concrete behavior without turning every task into model-building or
multi-agent search.
**Verdict:** Worth it.

### Prefer harness constraints over more prose when the runtime can enforce them

**Observation:** Schema's decisive properties are executable and observable.
**Cost:** Future product work: durable evidence ledgers, candidate verification,
and explicit side-effect gates.
**Benefit:** Prompts no longer rely on the model remembering to police itself.
**Verdict:** Highest-leverage follow-on, but outside this prompt-only change.

## Customer delivery

`PROMPTS.md` should remain author-time doctrine, not ambient execution context.
The direct readers are maintainers following `STYLE.md`, customers and agents
opening `/docs/prompts`, the website build that materializes that route, and a
compile-time contract test. Normal skill, Task, Project, and Wave launches do
not read the whole file.

Its runtime consequences travel through narrower surfaces:

- `LOOPFLOW.md` supplies the small evidence floor once per standard launch.
- Selected builtin skills supply methods only where they are exercised.
- `lf prompt` supplies a self-contained authoring workflow on demand for
  customer skills, directions, Wave goals, and inline prompts.
- Runtime tests and tools enforce boundaries that prose alone cannot.

This avoids multiplying hundreds of lines across every prompt while giving
customers one command and one public guide at the moment of authorship. A
future prompt compiler or linter could turn more of the contract into checks;
automatic inclusion of the whole guide would move in the wrong direction.

## Sources

- [Schema Harness](https://schema-harness.github.io/)
- [Schema trace release](https://huggingface.co/datasets/schema-harness/arc-agi-3-schema-traces)
- [OpenAI Cycle Double Cover prompt](https://cdn.openai.com/pdf/04d1d1e4-bc75-476a-97cf-49055cd98d31/cdc_prompt.pdf)
