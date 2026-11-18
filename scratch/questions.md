- `ops::human_session::tests::resuming_an_ended_conversation_launches_its_provider_as_the_session`
  passes alone and fails in `cargo test -p loopflow-execution --lib ops::human_session::`
  ("Native launch has no admitted invocation"), at the branch head before the
  main sync too (2026-11-17). Order-dependent: `journal::admit_process` returns
  early while another test in the same process holds the global `READ` /
  `EFFECT_FREE_READ` flags, so no Process is admitted. Nextest (one process per
  test, what CI runs) passes; CI's failure was the unpinned `LF_BIN`, fixed.
- Sync with main #1964: background Task conversations (`primary::hold`) now launch
  plain `lf session resume`; main's `--start-if-free` guard is gone with the flag.
  The `opened()` check before launch still skips a conversation someone already holds.
