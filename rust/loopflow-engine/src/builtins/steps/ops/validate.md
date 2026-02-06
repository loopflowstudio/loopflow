---
requires: none
produces: validation report
---
Validate flows, steps, and directions. Report broken references.

## Workflow

1. List all flows in `.lf/flows/` and builtins
2. For each flow, check that all referenced steps exist
3. Check that all referenced directions exist
4. Check that all referenced flows exist (for nested flow calls)
5. Report any broken references

## Checks

**Step existence.** Every `step:` or `- stepname` in a flow must resolve to an existing step file.

**Direction existence.** Every `direction:` must resolve to an existing direction file.

**Flow existence.** When a flow references another flow by name, that flow must exist.

**Circular references.** Flows should not create infinite loops.

## Output

Print validation results:

```
Validating flows...

✓ code/ship.yaml
  - implement ✓
  - compress ✓
  - gate ✓

✗ code/broken.yaml
  - implement ✓
  - nonexistent ✗ (step not found)

Directions:
✓ ceo
✓ designer
✗ missing-role (referenced in code/foo.yaml but not found)

Summary: 2 errors found
```

## Exit code

- 0 if all references valid
- 1 if any broken references found
