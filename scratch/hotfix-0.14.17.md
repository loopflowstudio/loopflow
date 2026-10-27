# 0.14.17 usage-update fix: decisions (Jack Heart, 2026-10-26)

- Release PR #1710 auto-merged at 04:49:58Z, so main is versioned 0.14.17 with a durable 0.14.17.001_release.sql (untagged, unbuilt).
- Jack chose: fix main and ship it as 0.14.17, including the captured_event index draft. The release tool rebuilds current main under the unspent version and re-prepares to canonicalize the new draft.
- The hotfix branch origin/hotfix/0.14.17-usage-update (e9b2c2eac1 on v0.14.16) is retired; it supplies the code to port.
- The prune's one-by-one session_events deletes are covered by the index.
