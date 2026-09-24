# Narrow provenance checkpoint — 2026-09-23

## Boundary

Continue `1d355199e` without discarding its partial implementation or subsequent
Task-local work. This checkpoint includes Task flow history, its focused store
and controller regression coverage, the planning architecture note, and the full
Task design with its passive native read contract. Later output-query, native
reader, CLI, and Swift changes remain in the working tree, outside this commit.

## Review

FlowPosition and its versioned worker claim remain execution authority. The
existing Task ledger retains the selected expanded plan and exact invocation,
step index, iteration, Run binding, transition, ready/blocked state, and settlement.
Every receipt is written in the transaction that changes the owning position.
Same-ID plan mutation rolls back; history never reconstructs a plan from YAML.

Worker binding occurs after the exact claim is matched. Human Session binding
occurs when its position is saved. Iterate preserves the actual return target;
retries preserve distinct Run bindings even when stage and iteration are equal.
Automatic retry through claim_task_worker also records the retry transition.
Completion, restart/replacement, and Work reopening retain settlement before
deleting the cursor. Stale operations and failed ledger inserts roll back.
Flow facts are excluded from Project/Wave observation delivery.

No further production change was needed after inspecting the completed dirty
provenance implementation. The focused store tests already cover the requested
repeated skills, Iterate/retry, restart, completed history, rollback, and parent
silence. Controller assertion changes account for the new inspection facts while
still proving prebind/interrupted failures introduce no spurious failure event.

## Passive native capture contract and limits

Rust reads provider-owned history using the recorded Session and exact account
or launch-recorded source location. Observation must never launch, resume,
replace, attach to, interrupt, or select credentials for a client. Native output
needs source/item identity and revisions, beyond journal `(run_id, event_seq)`.

- Claude: complete JSONL records from the account's exact Session transcript;
  content-block identities retain prose/tool relationships. Configured live
  arrival remains unproved in the inherited evidence.
- Codex: Session-matched rollout JSONL, including archived history; normalize
  response items once and exclude event mirrors/internal instructions. Inherited
  notes report a configured live prose/tool read with the client unchanged;
  this pass does not repeat or upgrade that proof.
- OpenCode: exact Session message/part rows from a read-only database; stable
  part IDs and revisions handle mutable output. Configured live arrival and
  arbitrary equal-count/backdated compaction remain unproved.

Partial trailing lines wait; malformed complete records, missing identities,
unavailable sources, and source resets must be explicit. A quiet readable source
is different from missing capture. Some autonomous summary journals lack full
tool results. These gaps prevent an all-sessions coverage claim; they are not
permission to narrow the accepted Task to autonomous output.

## Validation

- `cargo test -p loopflow --lib durable_store_tests --no-fail-fast`: 8 passed
  in this conversation, against the completed provenance changes.
- `cargo test -p loopflow --lib failure_releases --no-fail-fast`: 3 passed
  (the two affected Task controller regressions plus one matching harness test).
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.

These commands ran on the shared working tree, including the preserved later
output work; they are not a clean-checkout verification of the isolated commit.

No Watch UI, configured Watch demo, publication, or Task completion is claimed.
