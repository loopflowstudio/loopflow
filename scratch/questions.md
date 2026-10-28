# LOO-662 assumptions — 2026-10-27

Kickoff chose complete explicit document bodies in the task channel. Explicit
selection of an ambient file takes precedence over ambient same-path dedup;
ambient-only sources keep their existing refresh semantics. New resume selections
belong to the new turn; previously submitted docs remain in native history and
are not automatically replayed. These are reversible implementation choices in
service of Jack Heart's accepted no-tools delivery outcome.

Exact missing/unreadable/nontext file selections fail visibly; directory/glob
selection keeps applicable exclusions with a diagnostic for no usable files.
No consequential product choice blocks the plan.

The live proof found that exited standalone Sessions are retired, and a currently
attached Session refuses another input owner. Native resume verification therefore
uses a disposable repository conversation prepared by `lf connect --fresh
--prepare --json`, followed by two headless turns. This exercises existing native
history without changing Session retirement or attachment ownership.

Publication review preserves native skill metadata semantics: `docs:` frontmatter
belongs to Loopflow skills. Native skill acceptance uses CLI/config documents;
its fixture does not introduce a new native frontmatter extension. An explicitly
selected Wave goal is classified as Docs and supplied once in task text.
