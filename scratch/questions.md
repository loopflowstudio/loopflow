# LOO-347 assumptions

Jack Heart is not reviewing this Task; these were decided without him on 2026-10-01.

- **No new store.** The breakdown is derived on read from each capture's
  `context.json` and `events.jsonl`. "Records" is satisfied by the capture
  already retained; the only new write is steer attribution.
- **"Carried" is not pure resume history.** The record gives the provider's first
  request size. Subtracting the assembled input leaves resume history *plus* the
  provider's own preamble and tool definitions (about 44k on a fresh Claude
  step). They cannot be separated from local evidence, so the source is named
  `carried` and described as both. Codex reports no per-request input, so its
  carried value is unknown; peak comes from its usage checkpoints.
- **Units are not converted.** Assembled sources and tool output are cl100k;
  carried, compaction and request sizes are provider tokens.
- **Compaction is observed for Claude only** (`compact_boundary`). No local
  capture contained one, so the shape is taken from Claude's stream-json and is
  unverified against a real compaction. Codex has no recorded marker.
- **Steer authors are what the Steer record holds.** Linear comments become
  steers authored `user`; commenter names are not stored on steers, so the
  breakdown cannot name them.
- **Budgets use the existing keys.** Memory, scratch, goal and steers (goal
  budget) and the assembled total are flagged, resolved from the checkout's
  current config rather than the config at launch. Instructions, carried, tool
  output and compaction have no budget; LOO-346 owns stating them.
- **Earlier captures stay as recorded.** LOO-298's existing steps show steers
  inside `goal`, because their captures predate steer attribution. Its tool
  output, carried and peak values read correctly. New steps split steers out.
  Rewriting old captures was not attempted: they are immutable evidence.
- **Not demonstrated live.** The branch `lf` cannot open the installed store, so
  `lf usage --task LOO-298 --context` was not run end to end. The reader was run
  directly against two real captures (one Claude, one Codex).
- **Desktop is unjudged.** Session history gains one source line per step and a
  Task totals line; it compiles and the DTO decodes, but nobody has looked at it.
- `docs/lf-reference.md` rows were added by hand; the generator needs a catalog
  file whose producer was not found.
- Wave memory's code map still names `run_record.rs`, which no longer exists.
