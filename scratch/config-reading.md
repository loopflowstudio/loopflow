# Forgiving configuration files

Jack Heart: “and comments and other imparseable lines” and “we should also print warnings when we skip config lines”. On invalid known values such as `yolo: perhaps`: “Skip it and warn”.

Keep the work in PR 1886. Comments stay silent; unknown keys, invalid values,
ineligible file preferences and malformed YAML entries print warnings and are
ignored. Explicit setters/CLI/environment overrides stay strict. Malformed
entries are removed only from the in-memory parse, including indented children.
Warnings include file, entry and source line where available, never raw values.

Current repair: normalize `workflow` to `graph` before filtering. The CI
`two_people` failure reproduced locally before the fix and passed after it.
The original landing watcher was stopped. Its automatic ci-fix run also stopped
after a steer; it added the non-default personal/repository assertion and the
`two_people` gate guidance. No landing watcher is active now.

The new reader's four tests passed in a quick Rust probe using the actual source
and existing dependencies. Full `cargo test --lib config::` is running now; then
run the CLI bad-config test, two_people, Clippy, and publish the updated same PR.
Review tests expecting invalid file values to error against the new request.

Personal config: claude.model is absent until this lands and the new main build
is installed. default_agent is codex. QA and task/wave/repo sessions use Opus.
Fable overrides for kickoff/concept-review are commented, so they use Codex.
LOO-832 is abandoned; LOO-834 Fable fallbacks is filed and unstarted.

After merge: install the new dev-stream lf, set --global claude.model opus,
verify installed config/Graph preview and a small real Claude probe here.
