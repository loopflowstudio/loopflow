# Open choices (LOO-924 kickoff, headless)

- Name of the renamed stream trait: chose `Driver` (`claude::Driver`, …), because `Conversation` already names `transcript::session::Conversation`. Reversible rename.
- "A fifth harness is one file": with an enum it is one file plus one `HarnessKind` variant and one registry line. The plan says so instead of adding link-time registration.
- Sequencing: implement waits for #1953 (LOO-913). On 2026-11-16 it was CONFLICTING with main, because it predates the #1951 crate split.
- Implement pass 2026-11-16: not started. #1953 is still OPEN/CONFLICTING; `jack-heart/cli-prox` head 412da944f0 is a checkpoint mid-sync onto 606c13c2d7 (still has `rust/loopflow/src/agent/`, `rust/loopflow/src/harness/` paths) and is not on origin/main 470da475a1. It changes `agent/mod.rs` by ~1.6k lines, the file slice 1 deletes from. Resume implement once #1953 merges, or once cli-prox is synced onto main and this branch is stacked on it.
