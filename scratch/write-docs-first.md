# Write the docs first

Accepted direction — Jack Heart, 2026-11-03: exploration uses prototypes and
diagrams to choose a direction. A new write-docs step begins pursue, before
implementation. Its user documentation appears in Desktop through scratch;
review and editing remain available while implementation continues.

The draft is the working copy until implementation incorporates it into the
permanent documentation. Saved edits must reach implementation, and the draft
must not become a second maintained copy. No new review stop or Desktop UI.

No implementation work remains. Loops return to implement, preserving edits
instead of regenerating the draft. Internal changes need no invented user docs.
The implementation handoff includes this repository's override. Review uses the
existing Desktop scratch surface; no new UI or live file-edit notification was added.

Sources: [Mark Larah's user-docs-first pattern](https://blog.larah.me/write-the-docs-first/)
and [working backward from the finished experience](https://maven.com/articles/start-with-the-press-release-amazon-method).
Jack Heart explicitly chose user documentation, not a press release.

Check: graph:: (97), world_recipes:: (7), graph_discovery_tests:: (5), golden prompt regeneration, README/index sync, cargo fmt, and stable clippy --all-targets -- -D warnings passed.
