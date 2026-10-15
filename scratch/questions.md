# LOO-487 assumptions — 2026-10-15

- Jack Heart accepted implementation for hands-on review on October 15: use an
  overrideable `minimum-contrast = 3` and finite 160 ms cell emphasis using the
  existing motion token. Original-color mode remains `1`; final visual judgment
  awaits the demo.
- LOO-462's uncommitted copy/find work is a dependency, not source to transplant.
  Integrate its delivered implementation before changing those UI owners. Its
  separate Workflow-loading landing hold remains live; no parent repair is selected.
- Contrast preserves channel ordering, not exact hue or arbitrary color separation.
  Faint alpha, graphics, images and unknown transparent backdrops retain explicit
  limits. Review the fixture results before accepting the default.
