# Open questions

- `scratch/<branch>.md` was already cleared on this branch; implementation assumed Step 1 EC2 dogfood deliverables (deploy docs + remote smoke).
- `scripts/test_remote_smoke.py` now sends top-level `step`/`repo_root` for `/v0/sessions` (matching current API). If a remote host's first `/v0/repos` entry is not a loopflow repo with `.lf/steps/design.md`, should we require `--repo` explicitly or add repo probing logic?
