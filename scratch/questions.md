# Questions

- Kickoff chose truncation: when a relocated flag has no value, the reordered
  argv ends at that flag so Clap prints its missing-value error. Passed over:
  leaving the flag in place (`unexpected argument`), and making root flags Clap
  globals. See `scratch/plan.md`.
- `lf : fix the --repo flag` becomes a missing-value error rather than a
  prompt that silently loses words. Should an inline prompt keep root-flag
  words literally? Not changed here.
