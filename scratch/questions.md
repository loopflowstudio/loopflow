# LOO-710 assumptions and open questions

- **Archive publication.** Jack authorized GhosttyKit artifact publication to the
  `bin` R2 bucket on October 6, 2026 "including future updates through this
  workflow". The libghostty-vt archives ride the same `ghostty-vt-build` pipeline
  and bucket, so the 2026-11-02 implement pass published them under that
  authorization (an earlier note claimed publication, but the bucket answered 404
  for every target until this pass). Say so if a separate approval was wanted.
- **Snapshot path and reader are shared with LOO-709.** Per Jack's 2026-11-02 steer,
  this branch keeps `<Machine home>/screens/<session>.ghostsnp` (LOO-709's directory
  and key, this Task's format) and adds no DTO field; the writer is `transport::snapshot::leave` and the
  reader `transport::snapshot::last_screen` (decode plus `repaint`), used by
  `lf resume` when its stdout is a terminal. LOO-709's branch still carries its
  own `.vt` writer for the second-landing branch to delete (see the plan).
- **Owner ends not covered.** The snapshot is written on the lifetime policy's end
  (idle, executable removed or replaced) and when the program exits. A SIGTERM or
  SIGKILL from outside writes none; LOO-705's install supersession should end owners
  through the lifetime channel to get one.
- **Pruning.** Nothing removes a Session's `.ghostsnp` after it is painted; the file
  is overwritten by the next owner's end. Left to the idle Task's retention rules.
- **Desktop decoding.** GhosttyKit exposes no snapshot decoder; Desktop's pane gets
  the last screen through `lf resume` painting it, not by reading the file (LOO-672).
- **Colors (resolved, 2026-11-02).** Jack Heart's accepted fidelity requires
  explicit program colors. The formatter emits only tracked overrides; the
  viewer's default theme stays local. Cleanup resets program colors to that
  configured theme, matching the existing mode-reset boundary.
- **Alternate-screen preview ownership (resolved direction, 2026-11-02).** The
  earlier assumption that Claude's primary-screen behavior justified leaving the
  preview outside terminal cleanup is withdrawn. The accepted Session recovery
  behavior already requires cleanup through launch failure and live handoff.
  Transport now owns preview output and cleanup; a new owner supersedes the
  preview's modes, including when it draws nothing. Isolated PTY and CLI resume
  proofs cover failure, cancellation and handoff (see the plan). This is a repair
  implied by Jack Heart's brief, not a new product approval.
