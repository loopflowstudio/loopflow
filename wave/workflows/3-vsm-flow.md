---
asana_id: '1213883255378918'
linear_id: 852b8034-d57d-4210-9c1e-0be33e5a8cfc
notion_id: 32af8f99-3d81-81c7-8810-c1ce50f7962d
---
# VSM Flow

**Finish line:** `lf vsm` runs a single-pass viable system audit — s5 through s2 — against a chord-wave's members. Each step is a builtin. The flow produces code changes and ships in one PR.

## Context

All eight VSM steps are shipped as builtins (`vsm/s5-scan`, `vsm/s5-assess`, ..., `vsm/s2-scan`, `vsm/s2-assess`). Four govern flows exist (`govern-identity`, `govern-intelligence`, `govern-control`, `govern-coordination`), each running scan → assess → `wave/mutate` for one level.

What's missing is the top-level `vsm` flow that chains all four levels in a single pass. The govern flows run independently — `lf vsm` should compose them into one sequential audit producing one PR.

Algedonic signals are live: repair lineage, error classification, retry limit (3 attempts with backoff), and escalation to the attention queue all work end-to-end. The s3 control step can read real algedonic history and repair chain data.

**Remaining from algedonic work:** The CI webhook → ci-fix → escalation path exists but hasn't been proven against real GitHub CI. The mechanism is there — `ci_failure_handler` creates runs, repair chains work — but a live CI test would validate the full loop. Consider running that as part of proving s3.

## Flow definition

The top-level `vsm` flow chains all eight steps with a single `wave/mutate` at the end:

```yaml
# flow: vsm
- vsm/s5-scan
- vsm/s5-assess
- vsm/s4-scan
- vsm/s4-assess
- vsm/s3-scan
- vsm/s3-assess
- vsm/s2-scan
- vsm/s2-assess
- wave/mutate
```

Each step reads scratch/ from previous steps. Each can write code. The flow produces a single PR with all changes.

## Done when

- ~~Four builtin steps exist (s5, s4, s3, s2)~~ — shipped (8 steps: scan + assess per level)
- ~~Each step has access to member wave state and algedonic history~~ — shipped
- `lf vsm` runs them in order against a chord-wave (create the flow YAML)
- The flow produces actionable changes, not just reports
