# bugsbun: Migrate Maestro from lfpr to lfops

## Summary

Update Maestro's WorktreeService to use `lfops` instead of the deprecated `lfpr` command for PR and land operations.

## Changes

- `createPR()` now calls `lfops pr` instead of `lfpr create`
- `landPR()` now calls `lfops land` instead of `lfpr land`
- Renamed private helper from `runLfpr` to `runLfops`
- Removed unused `title` parameter from `createPR()` (lfops pr doesn't use it)

## Files modified

- `Maestro/Maestro/Services/WorktreeService.swift`
