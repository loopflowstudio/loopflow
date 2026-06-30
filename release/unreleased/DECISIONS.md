# Decisions

## 2026-06-30 — Local refresh and native deploy share one implementation

**Context:** Local development and the native Mac host had two implementations of the same operation: pull the default branch, rebuild `lf`/`lfd`, and install them into a local bin directory. The split made docs harder to explain and risked the Mac host drifting from the developer path.

**Decision:** `scripts/install.py` owns local installation. `install.py local --use` remains the full build-and-promote path for `lf`, `lfd`, and `Loopflow.app`; `install.py refresh` is the fast CLI-only update path. `scripts/pull-local-bin.sh` stays only as a compatibility wrapper, and native host updates call `install.py refresh` directly.

**Implications:** There is one implementation to test and one command family to document. Existing callers of `pull-local-bin.sh` continue working, but new docs point users and deploy scripts at `scripts/install.py`.
