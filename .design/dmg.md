# DMG

Adds DMG build and upload to the publish workflow. Maestro DMGs are uploaded to Cloudflare R2 alongside PyPI releases.

## Review

**Verdict:** Ready to ship

No significant issues. The implementation is straightforward: `--dmg-only` for standalone DMG releases, `--skip-dmg` to opt out during PyPI releases, and DMG steps integrated into the normal release flow. Error handling appropriately continues the release even if DMG steps fail (since PyPI is the primary artifact).

## Design notes

**Download URLs:**
- Versioned: `https://downloads.loopflow.studio/LoopflowMaestro-{version}.dmg`
- Latest: `https://downloads.loopflow.studio/LoopflowMaestro-latest.dmg`

**Environment variables for R2:**
- `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY` (required)
- `R2_BUCKET_NAME` (optional, defaults to `loopflow-downloads`)

**Remaining work (separate repo):**
- Website `/download` page at loopflowstudio
- R2 bucket and domain setup
