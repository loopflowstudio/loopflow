# Gate assumptions

- 2026-10-14: This process had no `UV_CACHE_DIR` despite the queue's inherited-cache direction. Restored `/tmp/loopflow-idsbugs-uv-cache`, the isolated path retained in `e3837c9c71:scratch/idsbugs.md`.
- The gate keeps upstream macOS test repairs bounded to expectations and the documented runner sandbox boundary. No production process-identity behavior changes, publication, or landing are part of this step.
