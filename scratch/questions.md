# LOO-665 assumptions — 2026-10-28 UTC

- Jack Heart's ban on manual credential-file rewrites excludes operator edits
  during diagnosis/acceptance. The hotfix extends the existing production
  activation writer with verified selection and preservation; it does not add
  a manual repair procedure or a new credential lifecycle.
- The unlabelled successful `pass56-mini-native-diagnostic.jsonl` receipt cannot
  establish which home worked. The labelled comparison and native-home receipts
  support the plan; no chronology of token rotation is inferred.
- General native refresh coordination is not selected. The implementation must
  nevertheless retain a version displaced during its own repair; compare-before-
  rename alone is insufficient proof of that preservation boundary.

- Implementation review found that a hardlink made before save-back would miss a
  provider's later pathname replacement. Codex different-account activation uses
  the same exchange at installation, preserving its switching behavior while
  retaining the actual outgoing inode. Claude activation remains unchanged.
