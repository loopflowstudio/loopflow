# LOO-691 questions

- Decided (Jack Heart, 2026-11-04): the host's installed `lf` performs releases.
  No separate release home. Jack's dedicated machine already follows `dev`, so
  release fixes reach it from green main, without the one-release lag.
  `release/README.md` should say the dedicated host follows `dev`.
- Assumed (LOO-691): an issue closed as a duplicate drops its hold, like not
  planned; the surviving issue needs the label to keep holding `stable`.
