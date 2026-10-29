# LOO-669 open questions

- **Website follows preview, download follows stable (slice 2, 2026-10-29).** The
  publisher still deploys the website for every release, so its docs and
  `/healthz` describe `preview`, while its install command and "Download for
  Mac" serve `stable`. Assumed acceptable for now (docs describe a superset);
  revisit if a stable-only flag confuses readers.
- **First stable move.** Latest today is the newest release (v0.15.x). Forward-only
  means stable stays there until a newer release is 7 days old; no backfill.
- **Stable advance gates on `completion: github-release`**, not on having a
  publisher, so publisher-only targets in other repos keep no Latest marker
  after `--latest=false`. Loopflow's target sets `github-release`.
