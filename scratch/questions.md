# LOO-304 implementation assumptions — October 5, 2026

The combined harness keeps copied observations and owned native execution in
separate Homes, joined only by its test transport. This preserves copied
identities and account/process references while permitting one fresh Task-bound
fixture in the same workspace. No copied Home identity or Session ownership is
rewritten. Baseline and candidate must include the same added fixture and sandbox
cost. This reversible implementation choice does not revise Jack Heart's budgets.

# LOO-304 gate-read assumptions — October 6, 2026

- The prepared fixture no longer exists. The new matched pair clones the
  surviving `/tmp/loo304-release-20261006/index-diagnostic.db` after verifying
  all 39 tables against the prepared manifest. It is a new dataset lineage by
  file identity; earlier cohorts are never compared with it.
- Baseline and candidate fixtures differ by one index, each matching its own
  binary's schema. The runner refuses its built-in comparison for that reason;
  the report states the table-equality proof instead of overriding the runner.
- Task details use four workers. Eight measured 30 ms faster for 280 ms more
  CPU; "quiet" won. Revisit if checkout observation becomes change-driven.
- The embedded draft schema changes development builds only. It is included
  because every branch measurement and every test process paid the replay.
