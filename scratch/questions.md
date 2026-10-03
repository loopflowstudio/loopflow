# Design review — 2026-10-02

Jack Heart's cleanup request preserves live history and requires design review.
The draft selects an offline capture conversion: active/unresolved writers block
without being stopped; existing prior-CLI fallback is insufficient admission fencing.
The feature review must accept that maintenance boundary before implementation
commits to it. No configured migration, installation, schedule change or release is
authorized. Reversible naming choice: `captures/<shard>/<artifact-key>` preserves
keys and depth; Session identity remains separate.
