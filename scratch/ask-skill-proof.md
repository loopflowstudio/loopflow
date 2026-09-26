# Skill-selected Ask contribution

2026-09-25. Bounded Ask contribution to LOO-295; no Task controller, transition,
skill, Flow, or user documentation edits.

## Behavior

`lf ask --skill unblock "Choose the delivery policy"` validates the named skill
using the existing loader in the caller's checkout, saves its name on the Ask
record, and launches the existing `lf skill unblock <question-and-session-guidance>`
path. The launch retains the existing cwd, model, Work attribution, and human
Session token. The Ask still waits for human completion; readiness does not
release its caller.

Reopening reads the saved name. Native provider resume retains the original
conversation; a fresh launch loads the named skill from the same checkout.
This persists selection, not an immutable snapshot of skill content. Existing
records without `skill` deserialize to `None` and retain inline prompt execution.
No new Session kind, lifecycle, database migration, or fallback skill was added.

Owned changes: `lf/mod.rs`, `ops/human_session.rs`, their focused tests, and this
note. The current command adapter lives in `lf/commands/ask.rs`, so a one-line
forwarding edit there was necessary instead of changing `bin/lf.rs`. This is
the sole edit outside the listed file scope; main should review it explicitly.

## Validation

Both focused commands passed (one test each):

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR cargo test -p loopflow --lib ops::human_session::tests::ask_skill_and_question_survive_reopening_and_reach_the_launch_prompt -- --exact
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR cargo test -p loopflow --lib lf::tests::cli_separates_ask_completion_from_flow_decisions -- --exact
```

The behavior proof writes and reads real Ask JSON in an isolated temporary Home,
rewrites readiness and rereads it, parses the actual launch arguments, and runs
canonical prompt preparation with a local test skill. The resulting prompt
contains both skill instructions and the question, preserves human completion
guidance, and retains the checkout. It also reads a legacy record with the skill
field absent and verifies inline Ask dispatch. The CLI test covers both ordinary
prompt-only Ask and the new option.

Formatting and whitespace checks passed:

```sh
rustfmt --check --edition 2021 --config skip_children=true rust/loopflow/src/lf/mod.rs rust/loopflow/src/ops/human_session.rs rust/loopflow/src/lf/commands/ask.rs
git diff --check -- rust/loopflow/src/lf/mod.rs rust/loopflow/src/ops/human_session.rs rust/loopflow/src/lf/commands/ask.rs
```

An initial compile caught a test using nonexistent `Commands::Prompt`; corrected
it to the existing `Commands::Inline`. The initial broad `ask_` filter was stopped
before tests ran and replaced with exact test names (it also matches `task_`).

Library Clippy passed with warnings denied:

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR cargo clippy -p loopflow --lib -- -D warnings
```

No full-suite or all-targets Clippy pass is claimed.

## Review and evidence limits

The implementation uses the existing named-skill dispatch and Ask record owner.
Missing skill content fails through the ordinary loader. Legacy absence remains
explicit rather than inventing an `unblock` default. Human readiness and completion
authority remain unchanged. The extracted launch-argument function is shared by
production and the behavior proof; no test-only execution abstraction was added.

No live Ask, provider, terminal, Task worker, installation, commit, push, or PM
mutation was performed. The proof assembles the launch prompt without spawning
a provider; it does not establish live native provider resumption or desktop
presentation. Concurrent edits in other files belong to main and were untouched.
