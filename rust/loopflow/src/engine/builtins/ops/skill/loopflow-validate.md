---
requires: none
produces: validation report
---
Validate flows and skills. Report broken references.

## Workflow

1. List all flows in `.lf/flows/` and builtins
2. For each flow, check that all referenced skills exist
3. Check that all referenced flows exist (for nested flow calls)
4. Report any broken references

## Checks

**Skill existence.** Every `step:` or `- skillname` in a flow must resolve to an
existing skill file.

**Flow existence.** When a flow references another flow by name, that flow must exist.

**Circular references.** Flows should not create infinite loops.

## Output

Print validation results:

```
Validating flows...

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
