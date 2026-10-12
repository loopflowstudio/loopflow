# Open design questions

2026-10-11 — Jack Heart's new-user compatibility direction.

- Is the proposed fallback merge order (squash, merge, rebase among allowed
  methods) the right lf default, with a repository-only preference when needed?
- Which actual fork-publication, sparse/submodule/LFS and setup-heavy repositories
  should define the next acceptance cases? Default-base integration now follows
  Git tracking; publication/stack/host assumptions still need a coherent cut.
- How should repository preparation checks be expressed without hardcoding
  `scratch-clear` or duplicating the host's required-check policy?
- Is GitHub enough for the first invitations? A second host needs identity,
  publication, review reads and completion together, not a command override.
- Does two-example parent inference match expectations for mixed-tool repos?
  What explicit placement control is needed when evidence conflicts?
- Separate-Git-directory root fallback avoids metadata but cannot recover an
  unrecorded primary checkout path; consistent Work identity needs a further probe.

The numbered concern selection is settled in the design and failure map.
