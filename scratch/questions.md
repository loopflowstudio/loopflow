# LOO-447 implementation assumptions

- October 10: a native Claude result UUID identifies one immutable result within
  its AgentProcess. Missing identity with an outstanding admission refuses;
  contradictory repeats retain the original receipts and leave later turns open.
  Atomic history correlation is a prerequisite, not the independent transport.
  Transport recovery must also order previously uncorrelated output; replaying an
  entire stream against current pending admissions would misattribute old output.
