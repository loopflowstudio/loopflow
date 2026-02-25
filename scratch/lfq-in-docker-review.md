# lfq-in-docker review

## What was implemented
- Added Python runtime tooling to `docker/lfd/Dockerfile` and installed the local `loopflow` Python package during image build, so `lfq` is available in the same image as `lfd`.
- Removed the deprecated `deploy/deploy.sh` helper script from the branch baseline.
- Updated remote deployment docs to use an explicit compose command with `--build` in:
  - `deploy/docker-compose.prod.yml`
  - `docker/.env.example`
- Tightened the image build step by switching to `python3 -m pip` with `--no-cache-dir`.

## Key choices
- **Ship `lfq` inside the lfd image** instead of requiring separate host Python setup. This keeps operational tooling colocated with the daemon.
- **Document compose-first deployment** instead of script-first deployment, because the old script was removed and drift-prone.
- **Use `--build` in deployment examples** to avoid stale binaries after pulling new commits.

## How it fits together
The image remains a multi-stage build: Rust binaries are compiled in the builder stage, then copied into a slim Debian runtime image. The runtime image now also installs the Python package from this repo, which provides `lfq`. Remote deployment guidance now points directly at the compose files already in the repo, with a rebuild flag to ensure new artifacts are used.

## Risks and bottlenecks
- Image size and build time increase due to Python + pip dependencies.
- Build now depends on Python package index availability at image-build time.
- Without a wrapper script, deployment now relies on operators running compose commands correctly.

## What's not included
- No new CI assertion that `lfq` exists in the built `docker/lfd` image.
- No new remote deployment automation script replacing `deploy/deploy.sh`.
- No changes to runtime auth or networking behavior.
