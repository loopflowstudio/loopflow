---
schema: 1
id: task-loop-trust
project_id: d19956b2-9955-437d-aea6-d91766231c77
stage: installed
instrument: lifecycle-scorecard
unit: ratio
target:
  at_least: 1
window: 7d
freshness: 30h
---

# Task loops earn trust

Fraction of Tasks settled during the trailing seven days that either
completed with every PR landed through Loopflow auto-merge or stopped with a
non-resumable failure receipt. Open Tasks are excluded. A user-landed PR or
manual Git repair inside the Task fails the metric.
