---
requires: none
produces: validation report
---
Validate graphs and skills. Report broken references.

## Workflow

1. List all graphs in `.lf/graphs/` and builtins
2. For each graph, check that all referenced skills exist
3. Check that all referenced graphs exist (for nested graph calls)
4. Report any broken references

## Checks

**Skill existence.** Every `step:` or `- skillname` in a graph must resolve to an
existing skill file.

**Graph existence.** When a graph references another graph by name, that graph must exist.

**Circular references.** Graphs should not create infinite loops.

## Output

Print validation results:

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

## Exit code

- 0 if all references valid
- 1 if any broken references found
