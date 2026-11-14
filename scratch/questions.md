# Open questions

## LOO-837: stacking on a published parent that is ahead of origin (2026-11-13)

Assumed: stacking pushes the parent's branch only when the parent has no
published PR. A published parent with unpushed commits is left alone and the
child bases on `origin/<branch>`, with the left-out commits counted in the stack
line. The other option is pushing through the commit fence, which revokes the
parent's Auto from the child's command. Detail in `scratch/loo-837-demo.md`.
