# Publishing and crate discussion — 2026-11-14

Jack Heart:

> ok so  I'm starting to think that there's really no need to publish Loopflow to crates.io. It's not a library. It's just to see a lot   given that, I think that if that's true (and I'm interested in your opinion), then does that mean that we should start breaking Loopflow up into separate crates? If so how much would that reduce the load as systems are building things and enable me to run multiple worktrees on my machine?

> And if so then how do we want to break up the system into crates?

The initial questions opened discussion of publishing retirement and crate boundaries.

Following the recommendation to drop crates.io publishing and proceed with a staged private-crate split, Jack Heart replied:

> yeah thi is all fine with me

Proceed with internal path dependencies and retire crates.io publishing with the corresponding release changes. The prior publishing-alternatives prototype is superseded. Start with one small extraction proving cross-worktree cache reuse, then definitions and storage. The proposed model, definitions, store, providers, terminal, runtime and application boundaries guide implementation; preserve the Graph's review/demo boundaries and measure performance before claiming savings. This direction does not authorize automatic merge.

Jack Heart added:

> we should probably reorg the rust in part to maximize the separation between crates and make it map to actual files edited together in real PRs

The proposed crate list is provisional. Use representative merged PR co-change patterns together with production dependencies to choose boundaries and reorganize code. Compare representative PR rebuild sets, distinguishing broad formatting and file moves from ordinary feature edits. Prefer compile-cost measurements to line-count estimates.
