---
asana_id: '1213883255378918'
linear_id: 852b8034-d57d-4210-9c1e-0be33e5a8cfc
notion_id: 32af8f99-3d81-81c7-8810-c1ce50f7962d
---
# VSM flow

**Finish line:** `lf vsm` runs a single-pass viable-system audit — s5 through s2 — against a garden wave's members. Each step is a builtin. The flow produces code changes and ships in one PR.

## Context

All eight VSM steps are shipped as builtins (`s5-scan`, `s5-assess`, ..., `s2-scan`, `s2-assess`). Four govern flows exist (`govern-identity`, `govern-intelligence`, `govern-control`, `govern-coordination`), each running scan → assess → mutate for one level.

What's missing is the top-level `vsm` flow that chains all four levels in a single pass. The govern flows run independently; `lf vsm` should compose them into one sequential audit producing one PR.

Algedonic signals are live: repair lineage, error classification, retry limit, and escalation to the attention queue all work end to end. The s3 control step can read real repair history.

## Flow definition

The top-level `vsm` flow chains all eight steps with a single `mutate` at the end:

```yaml
# flow: vsm
- s5-scan
- s5-assess
- s4-scan
- s4-assess
- s3-scan
- s3-assess
- s2-scan
- s2-assess
- mutate
```

Each step reads `scratch/` from previous steps. Each can write code. The flow produces a single PR with all changes.

## Done when

- `lf vsm` runs the shipped VSM steps in order against a garden wave
- The flow ends with one mutation pass instead of four disconnected PRs
- The flow produces actionable changes, not just reports
