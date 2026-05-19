# Canonical Asana waves — shipped

The configured Asana team is now the canonical source of truth for the wave
set. Design and forward-looking follow-ups are folded into
`wave/workflows/3-wave-discovery-and-root-chord.md` (discovery hardening) and
`wave/workflows/2-pm-round-trip.md` (strict mirror sync).

## How to verify

```bash
uv run python scripts/verify_canonical_waves.py   # repo override + canonical-team discovery (live Asana)
cargo test --all                                  # incl. discover_waves_* and merge_config_values_repo_overrides_*
swift test --package-path swift
```

Manual: launch Concerto on this repo. Delete `wave/root/` locally → "root"
still appears (Asana-backed). Add a bogus `wave/fake/fake.yaml` → "fake" does
not appear. Set a `repos:` override in `~/.lf/config.yaml` pointing at another
team → sidebar reflects the override team.
