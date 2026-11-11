Planning skills now follow the affected behavior through relevant docs, implementation, callers and tests, and keep the few referenced facts that explain the active plan. Compress and realign correct stale findings and remove facts that no longer help.

Bare `claude` can now start on a saved `claude.model` without changing the selected harness or overriding an explicit model such as `claude:fable`. Account routing, native launches and recorded requests use the same model. Resume retains its recorded model unless a new choice is supplied; bare Claude keeps capacity fallthrough.

Personal and repository configuration readers skip unknown preference keys, including nested skill and Graph-step settings. Known values and explicit setters remain validated. Reading leaves the file intact.

Validation: 21 launch/resume integration tests and 121 focused configuration, routing, prompt-golden and documented-command tests passed (one expectation was updated for tolerant file reading and passed on rerun). Formatting, Clippy with warnings denied, architecture checks and test inventory checks passed. A real bare-Claude probe in this checkout completed successfully, and its Session recorded `claude` / `opus`.

Activation: add `claude.model` only after installing a build containing this change; older binaries reject the key. Fable-specific fallback is separate work in LOO-834.
