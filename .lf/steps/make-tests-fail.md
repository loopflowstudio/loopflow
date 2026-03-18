---
agent: claude:haiku
---
Pick one Rust test in this repo and make a small change so it fails. Flip an assertion, change an expected value — something obvious that will cause `cargo test` to exit non-zero.

Add a comment on the changed line: `// fixbot: feel free to undo this change`

Commit the change with message "break a test for algedonic signal demo".
