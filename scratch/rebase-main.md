# Main integration — 2026-09-30

Resolved the existing Loopflow-owned sequencer onto pinned main `a6b1bc3df`.
All 178 commits replayed. Publication belongs to the waiting parent operation.

The branch's accepted Exec / AgentSession / FlowSession model remains in force.
Main's typed command discovery, command precedence, flattened step settings,
and short CLI guide remain in force too.

Concrete integration findings and repairs:

- Template inspection intercepted `flow list/show --sessions`. Saved inventory
  now reaches its SQL reader; template-only requests reject saved-inventory filters.
- Main's discovery resolver lost captured autonomous-step selection. A step
  carrying the saved boundary selects its captured Skill before mutable definitions.
- Added branch code still used removed `OccurrencePolicy` and `ConcreteOp`
  APIs. Constructors and readers now use main's flattened fields and Command type;
  persisted operation tags and the private saved-skill codec are unchanged.
- Ask launches escape reserved skill names. The focused proof uses `list` as
  the selected Skill and verifies its instructions reach the launch prompt.
- The old long CLI page conflicted with main's documentation split. Session,
  saved FlowSession, structured decision, and history contracts now live in
  `docs/lf-reference.md`; `docs/lf.md` retains the short workflow guide. Removed
  in-turn decision commands and per-Wave chapter rotation stay removed.

Focused behavioral evidence (isolated fixtures, no configured provider trial):

- `cargo test -p loopflow --test flow_discovery_tests --test cli_discovery`:
  11 passed.
- `cargo test -p loopflow --test session_cutover_tests a_taskless_step_records_its_decision_on_the_invocation`:
  1 passed; two captured steps consume their exact successful native completions.
- `cargo test -p loopflow --lib ask_skill_and_question_reach_the_launch_prompt`:
  1 passed.
- Required pre-commit checks: `cargo fmt --check` and
  `cargo clippy --all-targets -- -D warnings` passed; `git diff --check` passed.

The simulated review checked storage ownership, captured-source execution,
reserved-name dispatch, and documentation placement. The fixes above are its
concrete findings. Existing whole-design acceptance obligations remain in
`remaining-work.md`.
