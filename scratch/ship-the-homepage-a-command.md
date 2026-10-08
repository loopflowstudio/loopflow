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
Mac capture, why, install. Match desktop and mobile layout without review UI.
Keep the Mac download and the existing palette/fonts. Body mentions of lf use code.

## Delete — do not maintain

Remove the three-frame animation, home.js, its session/result images and exclusive
capture tests; remove levels/Discord and the separate autonomy comparison, their
helpers, content and styles. Keep wave-surface.png and provenance, the example
loopflow diagram, docs/download surfaces and their behavior.

## Review and remaining work

The renderer, copy and responsive stylesheet match prototype 05. Review found
Pico's inherited heading margins and inline-code boxes changed the layout; scoped
homepage styles now reset them. Terminal overflow stays within focusable blocks.
A same-page anchor jump is covered without assuming it makes a network request.
No child Wave memories exist under growth. Growth memory records the October 7
approval separately from September copy and preserves the missing-capture limit.

The cmux capture is still absent, as permitted by the directive. Download and
Source links retain production destinations. Publish the PR; landing/deployment
remain outside this directive.

Check: `cd website && uv run pytest tests/ -q` — 78 passed, 3 skipped (one visible mobile nav link); Ruff passed; `lf help --all` plus named help resolves every displayed command/skill; `lf screenshot` captured 1440/390, including full-page copies with loaded fonts embedded for deterministic capture; live document/body widths are 390; architecture output regenerated.
