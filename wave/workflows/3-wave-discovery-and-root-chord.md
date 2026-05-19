---
asana_id: '1213877387454414'
linear_id: 02536e4b-af6f-4c18-bd6c-7db0e00cd4c6
notion_id: 32af8f99-3d81-81e7-b1f5-f8d867892fa6
---
# Wave discovery hardening

**Finish line:** The canonical Asana wave set is stable across branches and degrades gracefully when Asana can't fence the set — no filesystem accidents, no silent empty sidebars, no per-repo override that requires editing config by hand.

## Shipped (this is the floor, not the work)

The configured Asana team is now canonical for the wave set. Every project in
`asana.team` is a wave; `wave/<name>/` is the local editing mirror, not proof of
existence. `discover_waves()` reads Asana team projects with a
`.lf/cache/workspace/projects.json` stale-cache fallback; `GET /v0/waves/discovered`,
`lf op pm list`, and Concerto all consume that one list. `asana.team` replaced
`asana.default_team` (serde alias keeps old configs parsing). `~/.lf/config.yaml`
`repos:` overrides merge on top of repo config. Team autocreate writes a
`Loopflow` team gid back to config for org workspaces. Filesystem auto-bootstrap
(`bootstrapRoadmapWavesIfNeeded`, `roadmapWaveNames`) is deleted.

Verify with `uv run python scripts/verify_canonical_waves.py` (requires live
Asana auth; documented in TESTING.md under Validation Scripts). Manual check: delete
`wave/root/` locally → "root" still appears (Asana-backed); add a bogus
`wave/fake/fake.yaml` → "fake" does not appear.

## What remains

1. **Branch-consistent config.** Today `asana.team` is read from the current
   worktree, so a branch that edits the config disagrees with main about the wave
   set. Read repo config from the `origin/main` tree instead. This is git-plumbing
   in the config loader with offline / fetch-freshness / fresh-clone implications —
   the reason it was deferred, and the highest-value remaining piece.
2. **Free-tier workspace fallback.** Asana workspaces with `is_organization: false`
   have no teams. Today discovery errors out with a clear message. Add
   workspace-as-fence fallback so non-org Asana plans aren't dead-ended.
3. **Local-draft decoration.** `wave/<name>/` dirs that aren't in Asana currently
   render as plain filesystem items. Decorate them as "local draft" so the
   editing-mirror-vs-canonical distinction is visible in Concerto.
4. **Override UI.** The per-repo `repos:` override is hand-edited in
   `~/.lf/config.yaml`. A Concerto affordance to point a checkout at a different
   dev team would remove the last manual-config step in the dev workflow.

## Done when

- Switching branches does not change the discovered wave set unless `asana.team`
  changed on `origin/main`
- A non-org Asana workspace discovers waves instead of erroring
- Concerto visually distinguishes Asana-canonical waves from local-only `wave/` dirs
- A per-repo Asana team override can be set without editing a YAML file by hand
