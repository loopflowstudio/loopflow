# Findings supporting LOO-427

Dated evidence; [plan](compare-cmux-s-command-line.md) owns scope and acceptance,
[questions](questions.md) unresolved judgments. October 8 removes the earlier
storage coupling locally.

## Earlier identity and storage findings

October 7's path-only identity, copied placement, mutable checkout joins and
single pending Desktop URL are superseded locally. Owner constraints and dated
observations: `f48d84511:scratch/findings.md`, this heading. Shared identity remains
independent of clone names/remotes; historical assignment provenance stays unknown.
LOO-406 owns local planning/Linear and LOO-412 exchange; runtime stays local.

## Decisions these findings do not establish

- Missing shared-source implementation does not reopen Jack Heart's accepted
  one-tree/delegated-subtree direction. Jack selected local planning with Git-ref exchange on October 8; implementation
  and integration are still required.
- Removing the former SSH `--repo` option did not ban Work-directed routing.
- Matching a parent's Machine is not evidence that a historical assignment was
  inherited. Migration must preserve the observed meaning rather than invent it.
- Exact pane/content identity prevents redirection, not destructive changes to
  an existing unsent draft. Session input needs the LOO-387 interaction contract.
- A successful command receipt does not prove usable rendering. The cmux
  disposable-window probe is evidence only for that host, not Desktop parity.
- `documented_commands` tests ambiguity; `dto_fixtures` tests wire shape. Neither
  replaces behavioral storage/routing/native-model tests.

Current alternatives rejected by the accepted direction: repository-pairing
registry; independent per-machine planning trees merged by Desktop; clone-name
or remote-URL matching as Work identity; rewriting Machine defaults per request.

## October 8 implementation review

Checkout location now lives on `tasks`, using the existing `TaskCheckout` reader.
Review found two paths outside that reader: SQL membership matched only paths,
and comparisons bypassed file-location validation. Both now use recorded Machine
evidence. Unknown Machine evidence previously allowed local file reads and
missing-path Session association; it now stays unavailable, preserving explicit
bindings. No new DTO or parallel placement store was needed.

The first focused run failed on fixture setup (missing required `input_published`
and a nonexistent PR branch); both fixtures were corrected. Gate still owns
materialized/installation migration and complete Session lifecycle verification.
This is slice-1 evidence, not shared-source or native Desktop proof.

## Integration boundary — October 8

No dirty dependency code or owner checkout was changed. LOO-406 through
`cbf0a174a` is integrated; LOO-412 exchange, delegation composition and peer start
admission remain. Seeded IDs prove routing, not exchange or exclusive first start.
Earlier stalled checks and the `b6f34a6f8` integration account:
`f48d84511:scratch/findings.md`, this heading. Updated dependency evidence follows.

### Integrated writer evidence — October 9

`cbf0a174a` is integrated, including `078a6642e`'s fixture repairs and
`2bab3fbd5`'s observed/unseen reopening regressions. Infrastructure memory records
Linux migration/Flow/work-watch reconnect and a 54-test planning pass including reopening; no
LOO-427 mixed Git/Desktop proof follows. `docs/architecture/planning.md` retains
the unconditional Linear-write race: acquisition/readback cannot exclude an
unseen concurrent reopen. The accepted causal-reopening outcome is unchanged.
Product has no child Wave memories. Related review covered Infrastructure planning only.

### Committed exchange update — October 8

Read LOO-412 at `ce740b028` through Git, without integrating it or changing its
checkout. `ops/planning_peer.rs` explicitly refuses mixed Linear/Git exchange;
its new foreground/post-save paths are not composed connected-repository proof.
`store/sqlite/planning_peers.rs::project_object` writes winner fields before
`acquire_linear_frontier`: rejecting a provider-cache update alone cannot undo
those projected fields. Infrastructure's memory records removal/archive refusal
and rolled-back unresolved-membership markers as remaining source findings,
not executed counterexamples. Grouped receipts and alternate acquisition remain.
The common-owner repairs must precede enabling that path; the selected Product
outcome still includes connected repositories and independent imports.

The dependency's user/shared refs and per-Wave destination selection supersede
this plan's provisional single-ref spelling. Joining selects/publishes nothing;
multiple destinations in one repository cannot become multiple Desktop windows.
LOO-427's local `repository_plans` binding still needs composition with that
selection, not a second exchange implementation.

Later committed source at `270019c8d` repairs the projection-order finding above:
`acquire_linear_frontier` precedes `project_fields`, and object-scoped SQLite
savepoints roll back rejected projection/cache changes. Removal/archive evidence
fences projection; typed membership and same-revision provider conflicts survive
outside the rollback so independent valid objects can import. Malformed provider
input still aborts the whole import. Task/Project regressions cover retained local
values, uncertain receipts, execution and repeat-import stability, but have not
executed. The dependency's memory reports SQL checks only. Mixed Linear/Git stays
disabled; grouped receipts, alternate acquisition, legacy association and Desktop
composition remain. This is inspection of committed dependency code, not integration
or a replacement sync implementation. No new product decision was
inferred from the dependency. Infrastructure was read only for these shared boundaries.

`a388ed425` routes peer Task disposition through common stable/causal receipts;
`7262b6b20` shares winner/baseline normalization. Inspected, not integrated;
SQL checks only, mixed Linear/Git still disabled. Grouped receipts, alternate
acquisition, legacy association and Desktop remain. Detail:
`fc87e09c1:scratch/findings.md`, this heading; the plan retains composition work.

## Arrangement evidence — October 8

`d9fa2b829` implements exact-target arrangement; `df3e43fb3` shares visibility/removal
pruning and removes duplicate reveal-before-focus calls. `f4b7aae11` adds companions;
`caa48b5c1` shares insertion and publishes command/focus/Undo state together. Source and fixtures retain
leaf occurrences, commands, document selection and view objects. Native terminal
drafts, responder behavior and the complete cross-machine path remain unproved.
The comparison tables remain the October 7 baseline; its source delta records
these additions without changing the original command judgments.

Arrangement checks: direct Swift 6 builds/typechecks and interpreted production smoke passed, including 31 insertion cases at `caa48b5c1`; packaged SwiftPM/test executables and Rust/Clippy stalled before results (30–120 s). Earlier failed attempts and commands are preserved in Git blob `939890436fc712a5e14875749d586ae3ae450523`. Gate/CI own packaged execution, demo owns native/composed proof.

## Terminal identity and extraction boundary — October 8

Terminal surface keys and callbacks now carry Machine identity, as do Session
reading/cache-marker keys. Review found that path-key qualification alone left
membership pruning able to remove another Machine's same-ID Session; pruning
and move evidence now include Machine too. Fixtures exercise that collision, wrong-
Machine close notifications, passive missing-surface inspection and delayed reads;
they typecheck, while packaged execution remains unavailable.
Inspection exposes `ProgramStatusSurface.incarnation` without allocation or
child-exit cleanup. Released views cannot create another surface under that token.

Pinned Ghostty's allocating `dumpTextLocked` cannot bound extraction by truncating
Swift's copy. `0004-bounded-text.patch` now adds caller-owned fixed storage through
the existing ScreenFormatter: byte bound, complete UTF-8 prefix, truncation flag,
empty success, and current text/command-block selection. UTF-8 repair examines
only the final scalar, removing repeated full-prefix validation. One byte-limit
matrix compares against the existing allocating reader, including empty output,
all scalar widths, combining characters and soft wraps. The Linux check also covers
oversized scrollback without allocation,
retained selection and stale command-block reads without cleanup. Review found
`selectedCommandBlock` released its stale tracked pin during a read; the getter
is now const, leaving release to selection changes/reset/deinit. Existing clipboard
and Quick Look reads still need their full text and viewport metadata.

This proves the terminal extraction primitive, not the embedded surface call or
Desktop path. Native Zig helpers stalled before startup, sampled at `_dyld_start`;
a cached build driver reached new helpers that also stalled. Linux lib-vt tests
ran without networking. The broader embedded Linux build could not resolve its
uncached HarfBuzz dependency; fetch-only preparation did not fetch that lazy
package. The new framework has not been built/published and Package.swift remains
on lf2. At this extraction-only cut there was no terminal-text CLI. The later
exact-target boundary below supersedes that gap: request transport and passive
validation now exist without the artifact. Native reads, exited/replaced surface
execution proof and bounded Swift copying still require composition. The later exact-input cut supplies `desktop text/key`; native input proof remains. The plan owns the check-result line.

Review also caught the native-soak fixture using its static synthetic Machine for
real disposable CLI records; its surface keys now use that record's workspace.
Direct module builds/typechecks and isolated model smoke establish compilation
and model behavior only; the plan's single check line retains timed-out packaged,
Clippy and full-library execution attempts. Native exit/no-cleanup and stale text-
read rejection remain unproved, not inferred from optional inspection tokens.


### Artifact build follow-through — October 8

The Linux application-root bounded-text check passes after explicit HarfBuzz
fetching. Both embedded libraries compile with `fontconfig_freetype` and export
the reader; bare `freetype` failed because upstream requires discovery. Package
fetches must reach the build's cache. This proves neither macOS ABI nor a running
surface. A minimal C executable also stalled at `_dyld_start`, before main;
the native framework remains unavailable, with no upload or manifest change.
Exact commands and failed attempts: `/tmp/loo427-input-skDK4w/scratch/findings.md`.

### Exact-target read boundary — October 8

`desktop read` now reaches passive surface validation through the existing Apple
event, router, registry and surface pool. Arrangement and reads share window and
pane validation. The pool bypasses allocating lookup and liveness cleanup;
native extraction remains explicitly unavailable under lf2. Contract fixtures
preserve empty versus missing, request identity and byte limits. A new real-surface
fixture is display-gated after review caught that placing it in the ordinary pane
suite would violate the headless check contract. It remains unexecuted. No
artifact, installed app or user terminal was changed. This advances slice 4,
not the single-PR acceptance boundary or Q2/Q3.

`5f4440e52` shares pane-to-terminal mapping and Apple-event replies, retaining
synchronous MainActor validation. It removes the fabricated-empty router fixture;
empty-output coverage is DTO shape plus the Linux formatter, not native extraction.
The missing-surface fixture covers each region without allocating a view. The related Infrastructure planning/peer sections were read, not its unrelated history.

## Scoped dispatch and preview — October 9

Repository validation had discarded its resolved ID, and command dispatch reparsed
raw argv, losing any correction. Both paths are replaced by carrying the selected
ID. The launch and explanation readers now share checkout-before-declaration
selection. Preview also exposed two hidden writes: opening an “existing” store can
migrate it, and bounding input wrote full-source files. Preview uses the shared
read-only registry; prompt assembly returns complete sources for launch to persist.
The oversized-message fixture proves exact system-input parity with a fake provider,
not a real provider account or remote preview. `--explain` is local identity reading,
not a complete action plan. The plan owns remaining work and the check-result line.
