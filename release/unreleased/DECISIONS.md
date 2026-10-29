# Decisions

## 2026-10-28 — Minor releases cut from main

**Context:** A minor had to equal the last patch's exact tree. Cutting v0.15.0, #1751 merged between the closing patch v0.14.20 and the minor's release PR, so the controller refused to tag; the minor was hand-prepared from main. The minor path also skipped prepare hooks, so generated inventory went stale on the version bump.

**Decision:** Jack Heart asked that a minor cut from current main like any release (LOO-669). It runs prepare hooks and ships whatever main holds, with notes covering the cycle since the preceding `.0`. The closing patch, the `.lf/releases/minor-<target>.json` receipt and the snapshot check are gone.

**Implications:** A minor can carry code that no patch shipped. Release streams (`dev` / `preview` / `stable`) give that code its soak instead of a frozen tree.
