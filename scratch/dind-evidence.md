# DinD Validation Evidence

## What we're testing

Docker Sandbox commands work from inside the bundled lfd container (Concerto's Docker-in-Docker path) with `/var/run/docker.sock` mounted.

## How to validate

After building the lfd container via `scripts/concerto-dev.py`:

```bash
# Preferred: one-command probe
uv run python scripts/concerto-dev.py sandbox-dind --container lfd-container

# 1. Verify sandbox plugin is available inside container
docker exec lfd-container docker sandbox version

# 2. Full lifecycle probe
docker exec lfd-container docker sandbox create --name lf-dind-test claude /tmp
docker exec lfd-container docker sandbox exec lf-dind-test -- echo "dind works"
docker exec lfd-container docker sandbox rm lf-dind-test
```

## Results

**Status:** BLOCKED (host Docker daemon unavailable)

**Date:** 2026-02-28
**Docker version (host):** 28.5.2 (client only)
**Docker version (container):** Unknown (could not exec into container)
**Sandbox version (container):** Unknown (could not exec into container)

### Probe output

```
$ uv run python scripts/concerto-dev.py sandbox-dind --container lfd-container
Validating sandbox lifecycle inside container: lfd-container
$ docker exec lfd-container docker sandbox version
Cannot connect to the Docker daemon at unix:///Users/jack/.orbstack/run/docker.sock. Is the docker daemon running?
```

### Decision

No DinD conclusion yet. Re-run after host Docker daemon is available, then choose:
- **Option A (preferred):** Add `docker-sandbox` plugin to the lfd Dockerfile
- **Option B:** Fall back to DockerExecutor when running inside DinD
