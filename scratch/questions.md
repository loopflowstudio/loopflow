# Questions

- **Mutable action tags.** Every third-party action in `.github/workflows/` is
  pinned by tag, not commit SHA (`actions/checkout@v7`, `astral-sh/setup-uv@v7`,
  `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`,
  `taiki-e/install-action@nextest`, `actions/upload-artifact@v7`,
  `actions/cache@v6`, `actions/download-artifact@v7`/`@v8`). An upstream that
  moves a tag reaches CI, the signed dev build and the release with no PR at
  all. Pinning to SHAs would close it; it is its own Task, not filed.
  Assumption taken: LOO-1120 covers Dependabot only.
- **Dependabot alerts stay on** (alerts only; security-update PRs are already
  disabled). Assumed wanted as the prompt for hand bumps.
- **Doppler also holds `DEPENDABOT_MERGE_TOKEN`.** The brief names the Actions
  secret and the PAT; the plan adds the Doppler copy to the post-merge cleanup.
