# Open questions

- Should Stage 01 be considered complete with the documented headless blocker, or do we require an iOS UI automation target that exercises `connection setup → connect → wave list → wave detail → live output` in CI?
- Should we harden local/CI macOS UI testing by setting a dedicated `DerivedData` path and splitting UI tests from unit tests to reduce intermittent linker/code-signing failures?
- Should macOS-only files live in `Concerto/Platform/macOS/` (this branch) or back in `Concerto/` root (main)? This is the primary rebase decision — see `scratch/jack-heart.mobile.20260224_2118.md` for full context.
