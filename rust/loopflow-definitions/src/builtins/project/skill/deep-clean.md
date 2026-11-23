---
requires: an area whose core design is settled
produces: a smaller codebase with the same valued behavior, and the list of what was kept and why
---
Delete what the area no longer needs, function by function.

This assumes the shape is right. If the core data structures or APIs are
what's wrong, stop and say so: that is `reduce`'s job, and cleaning code a
redesign will replace is wasted.

Read `scratch/` first. Your area is whatever the request names: a
subsystem, a surface, a directory, recent work. In a Wave with no request, it
is what the Wave's goal says it owns. Otherwise it is the whole codebase.

"We did a lot of work on accounts lately" names an area, not a diff. Find the
PRs that did the work, see which part of the product or infrastructure they
kept touching, and widen to the next cohesive whole. Work on that whole: the
PRs may have been patches on a larger problem. Open the output by naming the
area you took.

## Method

1. **Know what the product values.** Use the list `reduce` wrote if it is
   here; otherwise write the handful of behaviors this product exists for and
   what is questionable even though it works.
2. **Build the worklist mechanically.** Every function, method, class, field,
   parameter and command in the area, with its production callers and its
   test-only callers. Flag: nothing in production calls it; only tests use
   it; a parameter no caller overrides, or every caller passes the same way;
   a field no reader uses; a wrapper that only forwards; two definitions
   doing one job. Keep the script and the table in `scratch/`. The worklist
   is where the work starts, not a finding.
3. **Decide each flag:** delete, inline, merge, or keep. A whole module can
   go. A parameter every caller leaves alone becomes the behavior.
4. **Delete, with its tests and docs.** A test for deleted behavior goes too.
5. **Run every check the repository has,** not only the test suite: scripts,
   examples and CI-only checks import things too.
6. **Measure.** Lines before and after against the base, the twenty largest
   deletions, the kept-with-reason list, and what you left because it is
   risky, said plainly.

## How far one run goes

A run takes a large part of the area and finishes it: a whole subsystem, or
everything a family of flags touches across the area. Decided, deleted, checks
passing, measured, in a few coherent commits. One kind of flag or one module
is too small; cuts that belong together land together.

Then stop, with the worklist marked for what is decided and what is left, so
the next run takes the next part with a fresh context. A small area is one
run. It is done when every flag is deleted or kept with its reason.

## The test

"Is this called?" is too weak. Code can be called, tested and documented and
still produce behavior nobody cares about. The test is whether it serves
something the product values.

Keeping carries the burden. A kept item needs one line: a valued behavior, an
API the docs teach, a framework entry point, a reader for data people already
have, something tests must inject. A worklist where every entry ends "keep"
hasn't been done.

Some things only a person knows: whether anyone uses this provider, this
command, this mode. Ask those few questions together, and hold those deletions
until they are answered. Delete the rest without asking.

The result is smaller than where it started. If it grew, it isn't done.
