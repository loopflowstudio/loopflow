# LOO-710 assumptions and open questions

- **Archive publication.** Jack authorized GhosttyKit artifact publication to the
  `bin` R2 bucket on October 6, 2026 "including future updates through this
  workflow". The libghostty-vt archives ride the same `ghostty-vt-build` pipeline
  and bucket, so the 2026-11-02 implement pass published them under that
  authorization (an earlier note claimed publication, but the bucket answered 404
  for every target until this pass). Say so if a separate approval was wanted.
- **Snapshot path and reader are shared with LOO-709.** Per Jack's 2026-11-02 steer,
  this branch keeps `<Machine home>/screens/<session>.ghostsnp` (LOO-709's directory
  and key, this Task's format) and adds no DTO field; the reader is
  `transport::snapshot::read` plus `repaint`, used by `lf resume`. A comment on
  LOO-709 names both so its idle path calls `leave_snapshot` as the other ends do.
- **Owner ends not covered.** The snapshot is written on the lifetime policy's end
  (idle, executable removed or replaced) and when the program exits. A SIGTERM or
  SIGKILL from outside writes none; LOO-705's install supersession should end owners
  through the lifetime channel to get one.
- **Pruning.** Nothing removes a Session's `.ghostsnp` after it is painted; the file
  is overwritten by the next owner's end. Left to the idle Task's retention rules.
- **Desktop decoding.** GhosttyKit exposes no snapshot decoder; Desktop's pane gets
  the last screen through `lf resume` painting it, not by reading the file (LOO-672).
- **Palette.** The repaint does not emit OSC 4 so a viewer's terminal keeps its
  theme; programs that changed palette entries lose those on reattach.
- **Alternate-screen snapshot before the provider.** `lf resume` paints the
  snapshot straight to stdout, outside the viewer's `AlternateScreen` tracking. A
  snapshot taken on the alternate screen enters it in the person's terminal; a
  provider that never toggles mode 1049 itself leaves the terminal there on exit.
  Claude Code draws on the primary screen, so this is left as is.
