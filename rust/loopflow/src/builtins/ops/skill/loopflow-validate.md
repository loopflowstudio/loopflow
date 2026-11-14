---
requires: none
produces: validation report
---
Validate graphs and skills. Report broken references.

## Workflow

1. List every graph in `.lf/graphs/` and the builtins.
2. Check that each `step:` or `- skillname` resolves to an existing skill file.
3. Check that each graph named by another graph exists.
4. Flag graphs that call each other in an endless loop.

## Output

```
Validating graphs...

✓ task/build.yaml
  - implement ✓
  - compress ✓

✗ task/broken.yaml
  - implement ✓
  - nonexistent ✗ (step not found)

Summary: 1 error found
```

Exit 0 when every reference resolves, 1 otherwise.
