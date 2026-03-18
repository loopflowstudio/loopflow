# 05: Client Surface Cleanup

**Finish line:** Python package metadata and Concerto's shipped macOS surface match what Loopflow actually supports—no stale minimum version, no leaked demo/test windows, and no orphaned palette path.

## Carried context

- `pyproject.toml` still says `requires-python = ">=3.8"` while Ruff targets `py310`.
- `python/loopflow/client.py` handles `wave_logs()` errors inline instead of using the same `_raise_for_error` path as the rest of the client.
- `swift/Concerto/ConcertoApp.swift` registers `Terminal Test` and `Reply Demo` windows without `#if DEBUG` guards.
- `swift/Concerto/Platform/macOS/Views/ThemePreview.swift` depends on `.deepWine`, and `swift/LoopflowCore/Design/BrandColors.swift` still exposes that palette even though the main design system centers on light/dark.

## What to build

1. Align Python package metadata with the supported runtime floor already implied by tooling.
2. Collapse `wave_logs()` error handling into the shared client error path instead of keeping a bespoke branch.
3. Decide whether the demo/test windows stay debug-only or graduate into a supported product surface, then make the app registration match that decision.
4. Remove `deepWine` if it is just preview debt, or wire it into a real supported theme story if it is still intentional.

## Uncertainty

- `wave_logs()` streams lines, so the shared error helper may need a small streaming-friendly variant instead of a direct drop-in.
- If the demo windows are still useful for internal dogfooding, keep them available in debug builds rather than deleting them outright.

## Done when

- Python package metadata and lint/runtime expectations agree on the minimum supported version.
- Streaming wave-log errors surface through the same client error taxonomy as the rest of the Python API.
- Production Concerto builds do not expose test/demo windows or dead theme paths by accident.
