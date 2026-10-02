# LOO-369 decision schema rejection · 2026-10-02

Failed Session event 4293 records `loop-decide` failing with HTTP 400:
`invalid_json_schema`, because `maxProperties` is not permitted in
`codex_output_schema`. This is a provider request failure before a decision,
not evidence for Advance or Iterate. Repeating the unchanged request cannot help.

`rust/loopflow/src/engine/flow_output.rs::FlowOutput::schema` emitted that keyword,
a root-level `anyOf`, and optional explanation fields. The
[official structured-output contract](https://developers.openai.com/api/docs/guides/structured-outputs)
requires all properties, permits nullable fields, and disallows root unions.
`AgentConfig::output_schema` uses this shared schema at launch.

The local repair emits a closed object with required `decision`, `summary`, and
`reason`; the unused explanation is null. Settlement still rejects empty evidence,
the wrong explanation field, two explanations, unknown decisions and extra keys.
Existing stored receipts omitting the unused field remain readable. Router output
is unchanged. Authoring and architecture guidance now describe the emitted shape.

Review finding: deleting only `maxProperties` would leave two further incompatible
schema rules. The repair addresses all three without a provider-specific schema
rewriter or a new decision authority. No product requirement or review approval
from Jack Heart was added during this technical repair.

Next action: reassess with this repair, arrange for the caller's runtime to use
the rebuilt source, and explicitly recover the failed decision occurrence through
the existing Flow controls. Success means the provider accepts the schema and the
selected successful completion decodes; this Session does not choose that result.
No caller restart, runtime installation, provider replay, publication or Flow
navigation was performed. LOO-369 gate and Desktop demo remain pending as recorded
in `scratch/keep-current-tasks-visible-and.md`.

Check: `cargo test -p loopflow --lib engine::flow_output` — 4 passed; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` — passed; live provider acceptance remains unproven until authorized recovery.
