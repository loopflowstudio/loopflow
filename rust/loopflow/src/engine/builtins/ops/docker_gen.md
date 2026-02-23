Generate or update the repo's development environment.

The environment is defined by two files. Both must exist and be correct when you're done.

1. **`.lf/env-setup.sh`** — an idempotent shell script that installs everything the repo needs. Used by the Dockerfile at build time and by agents at runtime to update their live environment.
2. **`.lf/Dockerfile`** — thin wrapper: `FROM loopflow/agent:latest`, runs `install-loopflow.sh`, then runs project `env-setup.sh`.

## Workflow

1. **Fast path.** If `.lf/env-setup.sh` exists, run it in dry-check mode: compare what it would install against what's in the current `.lf/Dockerfile`. If `.lf/Dockerfile` is consistent, stop — nothing to do.

2. **Generate.** If no script exists, or it's wrong:
   - Read the repo: `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`, `Gemfile`, `build.gradle`, `CMakeLists.txt`, `.tool-versions`, `rust-toolchain.toml`, `.python-version`, `.nvmrc`, etc.
   - Understand what toolchains and dependencies the repo needs.
   - Write `.lf/env-setup.sh` — a single idempotent script that installs everything.
   - Write `.lf/Dockerfile`.

3. **Commit** both files.

## env-setup.sh

This is the real artifact. One script, idempotent, used everywhere:
- **Dockerfile** runs it at build time to create the image from scratch
- **Agents** run it at runtime after adding dependencies (their `update_env` command)
- Running it twice is a no-op — package managers handle this naturally

Start the script by delegating loopflow base setup:

```sh
if command -v install-loopflow.sh >/dev/null 2>&1; then
    install-loopflow.sh "$@"
fi
```

Structure it in order:
1. System packages (`apt-get install`)
2. Language runtimes (rustup, pyenv, nvm — only if not already present)
3. Project dependencies (`pip install -r`, `npm ci`, `cargo fetch`, `go mod download`)

```sh
#!/bin/sh
set -e

if command -v install-loopflow.sh >/dev/null 2>&1; then
    install-loopflow.sh "$@"
fi

# System packages
apt-get update && apt-get install -y --no-install-recommends \
    <packages> \
    && rm -rf /var/lib/apt/lists/*

# Language runtime (skip if present)
if ! command -v rustc >/dev/null; then
    curl --proto '=https' -sSf https://sh.rustup.rs | sh -s -- -y
fi

# Project dependencies
pip install -r requirements.txt
```

Each section is idempotent on its own. `apt-get install` skips already-installed packages. `pip install` skips satisfied requirements. Runtime installs check `command -v` first.

## Dockerfile

Thin. The script does the work.

```dockerfile
FROM loopflow/agent:latest
USER root
RUN if command -v install-loopflow.sh >/dev/null 2>&1; then install-loopflow.sh --install; fi
COPY .lf/env-setup.sh /tmp/env-setup.sh
RUN if [ -f /tmp/env-setup.sh ]; then sh /tmp/env-setup.sh --install; fi
USER agent
WORKDIR /workspace
```

## What matters

- **Correct versions.** Read version pins from manifests (`rust-toolchain.toml`, `.python-version`, `.nvmrc`, `package.json` engines). Install exactly those versions.
- **Idempotent.** Running the script on a fresh base image gives a working environment. Running it again changes nothing. Running it after adding a dependency installs only the new dependency.
- **Delegate baseline.** `env-setup.sh` should call `install-loopflow.sh "$@"` first when available, then apply project-specific setup.
- **Drop privileges after setup.** Build-time installs may need `USER root`; switch back to `USER agent` before runtime.
- **Fast re-runs.** Don't re-download or rebuild what's already there. Use `--no-install-recommends`, check before installing runtimes, let package managers deduplicate.

## What doesn't matter

- Multi-stage builds or image size optimization. This is a dev image.
- Pinning apt package versions. Pin language versions, not system packages.
- Supporting multiple OS variants. Debian only.

## Staleness detection

After writing the script, install the post-commit hook at `.lf/hooks/docker-check`:

```sh
#!/bin/sh
[ -x .lf/env-setup.sh ] || exit 0
expected=$(.lf/env-setup.sh --dry-run 2>/dev/null || true)
# If env-setup.sh changed, mark stale
if ! git diff --quiet HEAD -- .lf/env-setup.sh .lf/Dockerfile 2>/dev/null; then
  touch .lf/.docker-stale
fi
```

The executor checks `.lf/.docker-stale` to know it should rebuild the image before the next wave. Add `.lf/.docker-stale` to `.gitignore`.

## The script matters most

The Dockerfile is disposable — it just calls the script. The script is what agents run live to update their environment. Make it robust, readable, and correct. Future runs of this step verify the script still matches the repo and update it if not.
