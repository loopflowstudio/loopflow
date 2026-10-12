# Open design questions

2026-10-11 — Jack Heart's new-user compatibility direction.

- Is the proposed fallback merge order (squash, merge, rebase among allowed
  methods) the right lf default, with a repository-only preference when needed?
- Which actual fork, sparse/submodule/LFS and setup-heavy repositories should
  define the next compatibility acceptance cases? These selected concerns are
  not yet fully repaired by this cut.
- How should repository preparation checks be expressed without hardcoding
  `scratch-clear` or duplicating the host's required-check policy?
- Is GitHub enough for the first invitations? A second host needs identity,
  publication, review reads and completion together, not a command override.
- Does two-example parent inference match expectations for mixed-tool repos?
  What explicit placement control is needed when evidence conflicts?
- Separate-Git-directory root fallback avoids metadata but cannot recover an
  unrecorded primary checkout path; consistent Work identity needs a further probe.
- Which Wave owns the remaining work? Placement remains unresolved.

The numbered concern selection is settled in the design and failure map.
