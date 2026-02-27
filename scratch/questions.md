## Cross-repo portfolio edges

- Assumption: `GET /v0/repos` keeps existing wave-derived (unregistered) repo entries. Those entries now include `repo_id` by best-effort derivation from git remote, falling back to the repo path when derivation fails.
