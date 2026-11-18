# LOO-923 open choices (kickoff, 2026-11-18)

- Predecessors: the Task comment of 2026-11-16 says "Begin substantive implementation only after those measured outcomes are available." LOO-921 is idle on its shared-daemon question and LOO-922 is unstarted; LOO-923 was claimed and run on 2026-11-18. Assumed: the extraction proceeds and reports base-against-candidate numbers from `run.py`'s private caches; the comparison with LOO-921/LOO-922 results is added when they exist.
- First extraction is provider formats (`loopflow-providers`), not `ops`. `ops` as a crate downstream of execution would save more per ops-only edit (about 20 s of 57.5 s) but lengthens the chain. Is the one-crate-longer chain acceptable for that saving?
- Naming: format trait keeps `Harness` in providers; execution's launch half becomes `harness::Launcher`.
- `provider_auth` stays in execution for now because it also holds tracker credentials. Does it belong with providers, or split by what the credential is for?
