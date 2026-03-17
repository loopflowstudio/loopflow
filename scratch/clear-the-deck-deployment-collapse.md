# Deployment Collapse Validation

## Done when

- `docker/docker-compose.yml` defaults `LFD_AUTH_MODE` to `studio`
- `deploy/docker-compose.prod.yml` does not redundantly override auth mode
- `docs/lfd.md` opens with the two supported deployment shapes before the configuration reference
- `deploy/README.md` stays a recipe instead of duplicating env-var tables
- `docs/getting-started.md` and related entry docs point at `mode: container` instead of unsupported install flags
- `docs/lfd.md` presents Docker as the blessed container executor and keeps sandbox as a reference-only experimental override

## Measure

```bash
wc -l docs/lfd.md deploy/README.md docker/docker-compose.yml
```

Target: fewer total lines with the two-shape story front-loaded in the daemon reference.
