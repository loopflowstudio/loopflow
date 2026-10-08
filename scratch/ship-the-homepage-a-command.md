# Homepage prototype 05

Jack Heart approved prototype 05 on 2026-10-07 ("this looks great"). LOO-425
supplies the accepted copy and layout. Publication is the boundary; no landing
or deployment is authorized by this Task directive.

The original standalone prototype and passes.css were read without modifying
`loopflow.new-web`. Earlier committed website work from `origin/jack-heart/new-web`
was merged into main-based HEAD; conflicting architecture output retained main
until regeneration. Superseded research and drafts remain at `5e5d32d73:scratch/`.

Copy lives in content.yaml, section rendering in main.py, layout in style.css.
Order: hero, ownership, terminal Flow and three parts, four organizing rows,
Mac capture, why, install. Acceptance requires the approved desktop and mobile
layout without review UI, the Mac download, and existing palette/fonts.
Body mentions of lf use code.

## Implemented

Removed the three-frame animation, home.js, its session/result images and exclusive
capture tests; levels/Discord and the separate autonomy comparison, their helpers,
content and styles. Removed unused styles for earlier hero actions, install section,
video, product/terminal cards, code blocks and vocabulary cards. No deletion targets
remain. The Mac capture and provenance, example loopflow diagram, and
docs/download surfaces remain.

## Review and remaining work

The renderer, copy and responsive stylesheet match prototype 05. Review found
Pico's inherited heading margins and inline-code boxes changed the layout; scoped
homepage styles now reset them. Terminal overflow stays within focusable blocks.
A same-page anchor jump is covered without assuming it makes a network request.
No child Wave memories exist under growth. Growth memory records the October 7
approval separately from September copy and preserves the missing-capture limit.

Review also removed the generic dotted-path content resolver: the single homepage
mapping now supplies its sections directly. Mobile navigation tests check the whole
brand against navigation; separate logo checks duplicated that coverage, and link
pair checks always skipped with the approved single-link mobile navigation. Missing
brand/navigation now fails the retained desktop and mobile checks.

The cmux capture is still absent, as permitted by the directive; its absence
must be disclosed in the PR. Download and Source links retain production
destinations. PR #1496 is published for review. The final full-suite result is
recorded below. Landing/deployment remain outside this directive.
No product decision is pending.

Earlier implementation checks resolved every displayed command/skill with `lf help
--all` and named help, and captured 1440/390 with `lf screenshot`, including loaded
fonts embedded for deterministic capture. Live document/body widths were 390.

Check: `cd website && uv run pytest tests/` — 75 passed after final cleanup; `git diff --check 626789dcd0382c6754ba3b7c61ea2448b57658ce` passed; prior Ruff, CLI help and capture evidence retained.
