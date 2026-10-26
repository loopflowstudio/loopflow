Report this repository's lifecycle scorecard and name its first red signal.

Generate the scorecard from all retained Session history:

```bash
lf history usage --json --days 0 | python3 scripts/lifecycle_scorecard.py --history /dev/stdin
```

Add `--json` for the structured report. A nonzero exit is the result: report
the command and its stderr, and stop.

Answer with the scorecard's verdicts, then the first failing or unmeasured row,
what it measures and the evidence behind it. Change nothing; this is a report.
