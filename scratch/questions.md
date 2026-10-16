# Assumptions

- 2026-10-15: LOO-492 has no scratch design or screenshot attached here. Use its
  Task acceptance as the design and reserve the existing corner radius around
  terminal content. Preserve native grid padding because command-block overlays
  share those coordinates. The subsequent corner clarification is preserved in
  `corner-direction.md`. The known nested outlines belong to the workspace and
  its inset pane; their radii now share a center. Headless path inspection covers
  shape geometry; actual terminal frames and visual judgment remain display checks.

- Gate: use a 0.15-point tolerance for path-containment approximation around the
  5-point clear gap; the first measurement deviated by at most 0.09 points. This
  is subpixel geometry tolerance, not visual acceptance. The final test awaits
  capacity or capable CI; no resource-reserve bypass is authorized.
