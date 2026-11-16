# Open choices (LOO-924 kickoff, headless)

- Name of the renamed stream trait: chose `Driver` (`claude::Driver`, …), because `Conversation` already names `transcript::session::Conversation`. Reversible rename.
- "A fifth harness is one file": with an enum it is one file plus one `HarnessKind` variant and one registry line. The plan says so instead of adding link-time registration.
- Sequencing: implement waits for #1953 (LOO-913). On 2026-11-16 it was CONFLICTING with main, because it predates the #1951 crate split.
