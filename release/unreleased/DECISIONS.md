# Decisions

## 2026-10-28 — Minor releases cut from main

**Context:** A minor had to equal the last patch's exact tree. Cutting v0.15.0, #1751 merged between the closing patch v0.14.20 and the minor's release PR, so the controller refused to tag; the minor was hand-prepared from main. The minor path also skipped prepare hooks, so generated inventory went stale on the version bump.

**Decision:** Jack Heart asked that a minor cut from current main like any release (LOO-669). It runs prepare hooks and ships whatever main holds, with notes covering the cycle since the preceding `.0`. The closing patch, the `.lf/releases/minor-<target>.json` receipt and the snapshot check are gone.

**Implications:** A minor can carry code that no patch shipped. Release streams (`dev` / `preview` / `stable`) give that code its soak instead of a frozen tree.

## 2026-10-29 — GitHub's Latest marker is the stable pointer

**Context:** Every install path reads GitHub's Latest release (`install.sh`, `lf install`, the website, `lf machine`), and publishing marked each new release Latest, so every release reached new installs the day it shipped.

**Decision:** Jack Heart chose two pointers over one build line (LOO-669), with new installs defaulting to `stable`. Publishing now passes `--latest=false`. The daily release run moves Latest to the newest release published at least 7 days ago, forward only, after the publisher points `Loopflow-latest.dmg` at it. `preview` is the newest published release; `dev` is green main. Each release also ships `reader-catalog.txt`, the reader floors it opens, beside `SHA256SUMS`.

**Implications:** A release reaches new installs a week after it ships. Until `--preview` and `--stable` exist, getting a fresh release sooner means `lf install --main` or a pinned `install.sh --version`.

## 2026-10-29 — Regressions hold stable, Rust-style

**Context:** With `stable` a week behind every release, a bug found during the soak should keep the release that carries it from reaching new installs, without a separate hold command or holds file.

**Decision:** Jack Heart chose Rust's model (LOO-669): a regression is an ordinary bug Task marked with the release it regressed in, an explicit `regressed_in` field set with `lf task create --regressed-in v0.15.3` or `lf task edit`. The daily run skips every release the mark covers: from that version on while the Task is open, and up to the first release published after it completes. Status and Desktop show the mark.

**Implications:** Landing a fix releases `stable` a week after the fix ships. The field has no Linear mapping yet, so the release Machine reads only Tasks it holds locally.

## 2026-10-29 — Each Machine follows one stream

**Context:** With `stable` a week behind, installs needed a way to choose `preview` or `dev` and keep following it, rather than pinning a version.

**Decision:** Jack Heart chose stream flags (LOO-669): `lf install --dev|--preview|--stable` switches the Machine's stream and records it in `~/.lf/config.yaml`; `lf install` with no flag, the install schedule and `install.sh` follow it, `stable` by default. `--main` became `--dev`. `install.sh --channel stable|preview` does the same for a fresh install. A release whose reader catalog cannot open the store is skipped, not forced.

**Implications:** Switching to a slower stream after a newer lf migrated the store keeps the current lf until the slower stream catches up; no command deletes or downgrades the store.
