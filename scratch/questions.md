# Open questions and assumptions

- Codex service tier (Jack, PR #976 forced `serviceTier: "default"` so copied account
  config could not switch Sessions to Fast). Assumed the 2026-11-05 direction supersedes
  it: lf now sends no tier and Codex config decides. If explicit control is still wanted,
  the fix is a Loopflow `service_tier` setting that passes through only when set.
- Attended native Codex Sessions no longer receive lf's `workspace-write` floor; the
  docs already described the floor as headless-only, so the code now matches them.
- A wide `--lib` filter that spans `config::` and `harness::claude::` flakes three Claude
  harness tests (env-mutating fixtures run concurrently); each passes alone and together.
