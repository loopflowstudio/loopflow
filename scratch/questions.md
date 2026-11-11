# LOO-824 assumptions

Reversible choices made in kickoff; the plan is `holder.md`.

- `lf resume SESSION` (no `--task`) of a Session bound to a Task or Wave is
  managed. The brief names Task/Wave commands and Desktop only.
- Desktop Task shells stay on their own `session serve` owner: they run
  without a server on headless Linux.
- A graceful server stop writes a Ghostty snapshot per held Session so an
  install restores exact screen and modes; ring + nudge is the crash path.
- Ring 2 MiB per PTY; holder exits 60 s after its last child.
