# LOO-488 — decisions and remaining judgment

2026-10-15: Jack Heart approved the design with “looks good to me” after confirming
that LOO-488 stacks on LOO-462. Implementation proceeds through demo review;
landing, release, installation and running-app restart remain unauthorized.

Accepted:
- LOO-462's PR #1531 is the recorded parent and lands first.
- Two built-ins plus local RGB Ghostty imports; cream Light terminal and slate
  Dark terminal, preserving recognizable app identity. No Classic preset.
- Snapshot imports, explicit omitted-setting diagnostics and independently derived
  app semantic roles. Preserve low-contrast ANSI with a notice; renderer correction
  remains separate work.
- App-wide live preview with Apply/Cancel through Appearance; Desktop-local
  persistence and LOO-462's existing pane/navigation owners.

Remaining judgment: exact palette values and surface ladder, native coherence,
provider input/focus continuity and comfortable sustained use at demo review.
No blocking product choice remains before implementation.

Parent integration, 2026-10-15: synced LOO-462 through 7d10108eaf.
Its pane islands, program headers, attention queue, search and outer Work
multiplexer retain their existing owners. Jack Heart selected Ink on Paper and
authorized the parent’s full Swift prototype implementation, including mirrored
selection in Needs you. Command-click adds adjacent Work; ordinary click replaces
the focused slot; close hides while retaining panes. The sidebar is resizable.
See [Work multiplexer / Task workspaces](task-workspaces.md).

Cross-launch outer layout restoration remains outside the parent change. Its
reader watches focused Task Workflow detail; inactive workspaces retain live
panes and offer focus for actions. These accepted parent updates supersede the
earlier draft holds recorded at `eb793c2b1:scratch/`; they do not expand LOO-488’s
scope or authorize landing, installation or app restart. Native demo judgment
remains separate from implementation authorization.

Implementation follow-up, 2026-10-15: extend Ghostty's existing C getter for fixed
RGB TerminalColor values; unresolved cell-relative values remain unavailable.
Dim warnings estimate 50% opacity; personal settings and cell backgrounds can differ.

Parent integration assumption, 2026-10-15: preserve Jack Heart’s Ink on Paper
selection shape. Derive its burgundy/rose fill and primary/secondary text centrally
so dark and imported appearances meet the accepted contrast thresholds. Exact
colors remain demo judgment; fixed burgundy on dark surfaces did not meet 3:1.
