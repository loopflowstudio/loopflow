# Assumptions — 2026-09-30

- A nonempty Team list excluding the repository Team proves foreign ownership,
  even when several foreign Teams share the Project. Empty ownership remains
  unknown; a Project including the repository Team remains subject to validation.
- Wave sync skips foreign Projects in both inspection and rename execution, so
  a successful read cannot subsequently rename another repository's Project.
