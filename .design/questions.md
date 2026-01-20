# Implementation Questions

## Missing Design Doc

The `/implement` command was invoked but no design doc exists at `.design/<branch>.md`.

- **Branch**: `product-engineer/001`
- **Expected design doc**: `.design/product-engineer-001.md` or `.design/product-engineer/001.md`
- **Status**: Not found

The `/design` command should be run first to create the design doc, specifying:
- What to build
- Data structures
- Key functions
- UI changes (if applicable)
- Constraints
- Done when (verification command)

Without a design doc, the implement task cannot proceed.

## Next Steps

1. Run `/design` to create the design specification
2. Commit the design doc
3. Re-run `/implement` to build the feature
