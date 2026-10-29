# Decisions

## 2026-10-28 — Minor releases cut from main

**Context:** A minor had to equal the last patch's exact tree. Cutting v0.15.0, #1751 merged between the closing patch v0.14.20 and the minor's release PR, so the controller refused to tag; the minor was hand-prepared from main. The minor path also skipped prepare hooks, so generated inventory went stale on the version bump.

**Decision:** Jack Heart asked that a minor cut from current main like any release (LOO-669). It runs prepare hooks and ships whatever main holds, with notes covering the cycle since the preceding `.0`. The closing patch, the `.lf/releases/minor-<target>.json` receipt and the snapshot check are gone.

**Implications:** A minor can carry code that no patch shipped. Release streams (`dev` / `preview` / `stable`) give that code its soak instead of a frozen tree.

## 2026-10-29 — GitHub's Latest marker is the stable pointer

**Context:** Every install path reads GitHub's Latest release (`install.sh`, `lf install`, the website, `lf machine`), and publishing marked each new release Latest, so every release reached new installs the day it shipped.

**Decision:** Jack Heart chose two pointers over one build line (LOO-669), with new installs defaulting to `stable`. Publishing now passes `--latest=false`. The daily release run moves Latest to the newest release published at least 7 days ago, forward only, after the publisher points `Loopflow-latest.dmg` at it. `preview` is the newest published release; `dev` is green main. Each release also ships `reader-catalog.txt`, the reader floors it opens, beside `SHA256SUMS`.

**Implications:** A release reaches new installs a week after it ships. Until `--preview` and `--stable` exist, getting a fresh release sooner means `lf install --main` or a pinned `install.sh --version`.
