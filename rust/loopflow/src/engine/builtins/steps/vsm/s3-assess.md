---
requires: scratch/vsm-s3-scan.md
produces: scratch/vsm-s3-assessment.md
---
Assess control health and capacity.

## Goal

Turn live operating signals into control decisions.

Judge performance, identify mechanical blocks, and recommend how much parallel
work this chord can safely sustain right now.

## Workflow

1. Read `scratch/vsm-s3-scan.md`.
2. Judge health per wave: throughput, failures, stalls, retry churn.
3. Identify mechanical blocks that s3 could fix directly.
4. Recommend a worker-pool / concurrency level the chord can absorb.
5. Produce pressure points specific to health and resource allocation.

## Output

Write `scratch/vsm-s3-assessment.md`:

```markdown
# VSM S3 Assessment — <date>

## Summary
<overall control health and the main capacity constraint>

## Health by Wave
| Wave | Health | Evidence | Pressure |
|------|--------|----------|----------|
| ... | ... | ... | ... |

## Capacity
**Worker pool recommendation**: <N>
**Why**: <reasoning tied to real signals>

## Mechanical Blocks
<what is blocking execution right now>

## Pressure Points
1. <highest-leverage control concern>
2. <second>
3. <third, if needed>
```

## What to avoid

**Fake precision.** Use the real signals you have.
