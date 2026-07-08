# Decisions

## 2026-06-30 — Local refresh and native deploy share one implementation

**Context:** Local development and the native Mac host had two implementations of the same operation: pull the default branch, rebuild `lf`/`lfd`, and install them into a local bin directory. The split made docs harder to explain and risked the Mac host drifting from the developer path.

**Decision:** `scripts/install.py` owns local installation. `install.py local --use` remains the full build-and-promote path for `lf`, `lfd`, and `Loopflow.app`; `install.py refresh` is the fast CLI-only update path. `scripts/pull-local-bin.sh` stays only as a compatibility wrapper, and native host updates call `install.py refresh` directly.

**Implications:** There is one implementation to test and one command family to document. Existing callers of `pull-local-bin.sh` continue working, but new docs point users and deploy scripts at `scripts/install.py`.

## 2026-06-30 — Release notes fuse decisions and shipped behavior

**Context:** Cadenza's first release showed the useful half of the decisions ledger — it captured intent — and the weak half of a raw ledger dump: it was too long and not interpreted. Loopflow's older PR-based notes had the opposite problem: concrete changes without enough narrative intent.

**Decision:** The shared `release-notes` skill now treats `DECISIONS.md` as the intent ledger and merged PRs/diffs as the behavior ledger. `lf op release notes` uses the same agent-backed release-note skill as `lf op release run`, so standalone notes, weekly releases, and repo consumers share one prompt contract.

**Implications:** Release notes should read as an interpreted story grounded in shipped behavior, while the raw decision ledger remains archived under `release/v<version>/DECISIONS.md`. Repos like Cadenza can call Loopflow's release-note command instead of embedding their own note writer.
