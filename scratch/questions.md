# W2-174 — open questions / decisions carried to next serial PR

## Verified on main
- Repeated-failure rollups (contract item 4) are DONE on main via #934:
  `attemptFailurePresentations` collapses equivalent operational-only failures
  (same flow/step/reason, no prose) into one notice carrying every attempt +
  count + latest age; `visibleConversationTurns` drops the duplicates and never
  collapses authored prose. No further rollup work needed.

## Next serial PR scope (carried from W2-174 contract)
1. Supervisor narration curation. Grounded in the real Product Wave journal
   (/Users/jack/src/loopflow/.lf/journal/waves/product/journal.jsonl, 24k lines):
   each wave turn interleaves `stream` prose with hidden `command` tool calls,
   then ends with a trailing conclusion. Plan: surface the concluding prose;
   move the intermediate tool-interleaved operational narration behind a
   disclosure ("steps"/"reasoning"), same shape as the failure Details rollup.
   Raw record untouched (journal keeps every segment). Conservative: only split
   when the turn has tool-call boundaries AND a distinct trailing conclusion.
2. Typed Project + evidence references. Real narration names Projects/evidence
   in prose with no unambiguous identifier ("Loopflow API Project",
   "generation 2"), so — per the directive — define the smallest AUTHORED
   typed-reference contract instead of excluding them:
   - `project:<slug>` -> Project reference (slug is the PM identifier).
   - `evidence:<token>` -> Evidence reference (opaque; disclosure-only popover).
   Extends the ChatReference parser + ReferenceTextView from PR #947.
3. Proof must include the REAL Product Wave history rendering readably, not only
   parser fixtures.

## Do NOT
- Do not mark W2-174 complete until the above ship.
- Do not strip authored prose with content regexes (violates
  WaveChatTranscript's "prose and decisions are the conversation").
