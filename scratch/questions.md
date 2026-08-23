# Assumptions

- Treat non-ignored untracked files as review candidates alongside tracked
  files. This preserves the checker's useful pre-stage detection while Git's
  standard excludes remove local runtime material. Revisit only if Loopflow
  deliberately changes architecture checking to an index-only gate.
