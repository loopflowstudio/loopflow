# LOO-447 implementation assumptions

- October 10: a native Claude result UUID identifies one immutable result within
  its AgentProcess. Missing identity with an outstanding admission refuses;
  contradictory repeats retain the original receipts and leave later turns open.
  Atomic history correlation is a prerequisite, not the independent transport.
  Transport recovery must also order previously uncorrelated output; replaying an
  entire stream against current pending admissions would misattribute old output.

- October 10: the draft assumes a native Claude client can consume the proposed
  stream transport. No such bridge is identified; native `attach` targets
  background sessions, not `--print`. Preserve the accepted takeover outcome,
  but leave lf-owned terminal presentation versus native-supervisor adoption
  unresolved rather than silently changing execution ownership. See the plan's
  public-client mismatch and primary sources.
