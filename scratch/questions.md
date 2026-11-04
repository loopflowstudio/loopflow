# LOO-691 questions

- Decided (Jack Heart, 2026-11-04): the host's installed `lf` performs releases.
  No separate release home. Jack's dedicated machine already follows `dev`, so
  release fixes reach it from green main, without the one-release lag.
  `release/README.md` should say the dedicated host follows `dev`.
- Assumed (LOO-691): an issue closed as a duplicate drops its hold, like not
  planned; the surviving issue needs the label to keep holding `stable`.
- Assumed (LOO-691): `lf release issues <version>` lists the issues labelled
  with that version and the earlier regressions the published release still
  carries, so a held release never reads as clean. Closed-as-not-planned issues
  stay out: they hold nothing.
- Assumed (LOO-691): `lf release regress` writes the body "Regressed in vX.Y.Z."
  because `gh issue create` needs one without a terminal, and takes no `--target`:
  labels are repository-wide.
