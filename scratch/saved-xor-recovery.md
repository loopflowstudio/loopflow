# Saved XOR recovery and continued integration

Accepted constraints: preserve recoverable facts; lost execution continuity is
preferable to crashing. Never reload mutable definitions and call them the old
captured invocation. Do not turn recovery into approval of a human boundary.

User experience before implementation: upgrading over an old unresolved XOR
must leave Wave history readable and unrelated Sessions usable. An affected
invocation explains that its old branch body was not captured and names the
explicit restart path. Existing historical Wave restart machinery may handle
that disposition; no second migration lifecycle is needed. Original journal
and position files remain available as evidence.

Observed: full capture changed ConcreteXor to contain a Skill and captured
ConcretePaths. The existing Wave historical step decoder does not recognize
the previously shipped unresolved XOR shape. Journal::open consequently rejects
an otherwise complete journal, while read-only folding truncates at that line.
Ordinary Flow Session listing propagates one unreadable position's error and
therefore hides every other Session. Task XOR was rejected before this branch.

Plan: extend the existing historical Wave step disposition to recognize that
specific saved shape without source reads. Preserve subsequent journal events
and existing explicit restart semantics. Isolate unreadable ordinary positions
during Session enumeration, warn with invocation/path and actionable recovery,
and make direct reads report the exact saved position. Do not swallow arbitrary
errors during a targeted resume or overwrite failed parses.

Proof: populated old XOR journal plus later messages reads fully without
rewriting bytes, remains non-executable until the existing explicit restart;
one unreadable ordinary Flow alongside a valid waiting review leaves the valid
Session visible and preserves the old bytes. Existing current captured bodies,
human authority and nested traversal must still pass.

The local upstream rebase completed cleanly. Migration check now passes against
v0.12.21, with all 52 shipped migrations unchanged. No installed binary changed.
Two bounded lf agents own cursor deduplication and docs/skill integration;
main owns the recovery paths and combined validation.

## Results and subsequent audit

Both targeted regressions first failed against the actual readers:
the historical journal failed at its old XOR record, and ordinary listing
failed while parsing one old position. The first repaired run exposed a test
fixture error in the later message envelope; correcting that fixture let the
test exercise retention of the whole journal rather than merely its first line.

- `7b8694fe-2532-4653-a3a0-b073be49c6b7`: both the old-XOR journal proof and
  real WaveRuntime explicit restart/reopen proof pass. Original journal bytes
  remain a prefix after restart, and queued intent is retained.
- Ordinary Session isolation and nested human review passed in the preceding
  four-test run (whose journal fixture failed); no broader pass is claimed
  for that intermediate run.
- `512ed332-29ea-4ef8-95ea-f7244e1686df`: six Ask/recovery proofs pass. A
  deterministic pending-lock test establishes that reopening cannot overwrite
  a completed answer. Malformed native Run metadata cannot prevent durable
  completion and release of the waiting caller. Failed/interrupted decision
  Runs lose blocker authority. Existing keyed joining and launch retry pass.
- `0d1a4036-c964-4d88-9482-6626d9ac9f15`: three final Task integration proofs
  pass after cursor deduplication: nested human decisions, persisted exact route
  and verdict ownership, and the two-pass fresh-Run driver/recovery fixture.
- Bounded agent contributions passed 15 execution tests and 19 builtin,
  expansion, Task-policy and Ask-prompt checks; their scoped evidence is in
  cursor-dedup.md and concept-integration-audit.md. These counts overlap and
  must not be summed as distinct coverage.

The Ask audit also identified an artifact/Home mismatch when a development
binary launches PATH's installed lf with its Home-local ID. The isolated real
CLI regression first reproduced the wrong advertised binary. After the fix,
uninstalled development processes continue through their own executable;
installed artifact selection still follows promotion, and the old control pin
is never restored as launch authority. scripts/dev-lf explicitly pins the built
binary for children. The CLI test clears both binary overrides, places another
lf first on PATH, then executes the advertised reopen command and reads the
same Ask. `503ccb28-b36c-41b9-abdb-938a434d4fa6`: that real CLI proof and ten
process-selection/authority tests all pass. No provider, production installation
or production state migration was involved. Live native Ask/human acceptance
remains a separate gate.

Final review: all three concrete Ask audit findings are repaired, with proof
at their relevant boundary. Reopening does not hold a file lock throughout a
native conversation; it only resets missing history after a locked reread of
the same waiting record. Completion is durable before teardown; cleanup errors
are reported without revoking the answer. No new decision protocol, recovery
store or scheduler was added. Broader Task/Linear state restoration remains
LOO-296; these repairs keep the new Flow/Ask paths from introducing more loss.

Final all-target Clippy with warnings denied, cargo fmt check, diff whitespace,
architecture ownership, migration history and dev-lf shell syntax checks pass.
No push, install, merge, live provider/Ask acceptance or full-CI pass is claimed.
