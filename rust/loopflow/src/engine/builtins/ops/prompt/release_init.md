Bootstrap release infrastructure for this repository.

Inspect the repo and propose the smallest complete release setup that can ship a tagged release.

## Requirements

1. Detect languages and package manifests in this repo.
2. Propose a GitHub release workflow under `.github/workflows/`.
3. Ensure source manifests hold real versions (no CI-time `sed` patching).
4. Add or update `.lf/config.yaml` release target configuration when needed.
5. Create any missing release notes scaffolding used by the workflow.
6. Prepare the repo for first release tag creation.

## Constraints

- Prefer the fewest files and simplest workflow that works.
- Keep existing CI patterns unless they block releasing.
- Do not ask questions; make best assumptions and proceed.
- If uncertain, choose reversible changes and explain tradeoffs in commit/PR text.
