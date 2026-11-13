# LOO-887 open choices

- The "≤ 35 tests" target counts what CI runs: `transport_tests.rs` now has 34
  non-ignored `#[test]`s. Eight ignored acceptance tests (Rex ×3, mobile ×3,
  launchd ×2) stay as the plan says ("untouched"), and the 3 SSH tests sit in
  `transport/ssh_shells.rs` (ignored on macOS, run on Linux). Counted with the
  ignored ones the file holds 42. Getting under 35 that way would mean deleting
  or folding the acceptance tests, which the plan keeps.
- Several merges are legs: one `#[test]` calls two or three cases, each with
  its own `Fixture` (launcher exits, JSON launches, takeover, headless,
  continuity, pipes, directory). The count drops, and CI time drops only by
  the libtest overhead. Sharing one fixture would need one provider script per
  case; not attempted.
- Relay case `cancelled` (SIGINT while waiting for an acknowledgement) was not
  ported. Exit 130 comes from `lf`'s `ctrlc` handler (`agent::exit_on_interrupt`),
  not the relay, and "no input before acknowledgement" is the
  `unacknowledged-owner` case.
