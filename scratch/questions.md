# Open questions and assumptions

- The pre-PR-history schema records that a PR merged but does not record its merge
  SHA. The forward migration stores `legacy-unknown` for those already-merged
  rows so it preserves the known state without inventing a commit hash. Every
  merge observed after migration must store the real provider merge SHA.
