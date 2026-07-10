# Assumptions

- Product performance had no existing latency thresholds. This clarification
  sets user-visible p95 budgets at 1 second for state-bearing paths and 2
  seconds for terminal attachment; dogfood data should tighten or correct the
  thresholds rather than leaving the KR unnamed.

# Blockers

- The live Linear backlog could not be audited on 2026-07-10 because both
  `lf pm show --wave product` and `lf pm sync --plan` fail against Linear's
  current schema: their GraphQL variables use `ID!` where Linear expects
  `String!`. This is product implementation work, not a charter edit.
