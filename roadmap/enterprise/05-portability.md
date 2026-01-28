# Enterprise Roadmap: Full OS Portability

Make Loopflow behavior consistent across macOS and Linux.

## Goal
Ensure identical behavior on supported OSes to enable managed clusters.

## Scope
- File watching semantics
- Process and signal handling
- Path and filesystem assumptions
- Dependency footprints

## Success criteria
- A documented OS support matrix.
- CI runs on both macOS and Linux.
- No macOS-only assumptions in core flows.

