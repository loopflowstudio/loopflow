# Integrated workspace review — 2026-09-23

The slice advances the accepted design; publication remains withheld. The
configured Task conversation, exact draft, completion/shell return and terminal
viewport receipts remain valid at their recorded binaries. This review adds a
passing mounted native proof of Session-row return across two checkout groups.
The corresponding configured trial stopped before interaction because this exact
runner lacks Accessibility permission. No new production defect was established.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Unified planning | Compact ranked Tasks, directive/KRs, explicit missing evidence and exact Session access | Shared Podium readings and typed projection remain the only inventory path | Complete diff, source trace, prior configured navigation | pass at recorded levels |
| Configured conversation | Task context and configured destination; preserve conversation and usable shell | Prior owned Task launch, exact submitted draft, Complete and shell response | Iteration 8 review provider history/CLI/UI receipts; unchanged production source | pass for Task/terminal destination |
| Exact row return | Select embedded Session without opening or replacing another provider | Row restores its attached shell, native focus and companion layout in the correct checkout | New mounted SessionsView test, fixture Sessions and four real PTYs | pass locally; configured gap |
| Nested workspaces | Two inner layouts survive outer splits, selection and hiding | Two groups remain independent; selecting an already-visible group focuses its slot; hide/return preserves draft and children | Same focused native test through actual row and Split worktrees controls | pass locally; configured gap |
| Scroll retention | Preserve terminal viewport during navigation | Prior configured screenshot sequence retains history rows 1–33 versus bottom 168–200 | Iteration 9 receipts; unchanged production | pass for that configured path |
| Appearance | Readable titles and controls in both appearances | Window appearance and custom palette align | Re-inspected installed counterexample and integration light/dark captures | pass for recorded integration rendering; installed app not refreshed |
| Shared ownership | One planning/Session reader, existing workspace owners and exact terminal attachment | No removed navigation/reader types returned; PTY-bound terminal IDs remain distinct from Work attribution | Negative searches and Rust/Swift path review | pass, source; LOO-284 contract still outstanding |
| Remaining slice proof | Configured row/nested interaction, scope/destination coverage and comparable timings | No new successful configured interaction here; bounded prior timing observations remain | Exact runner permission failure below | gap |

## What was reviewed

Recovered the full Task diff through `lf task diff LOO-291 --json` at starting
HEAD `56078324355f5eb208767cf9c62954d3dbb58e5d`: `truncated: false`, 984,613
patch characters. Receipt `/tmp/loo291-review-integration.json`; extracted patch
`/tmp/loo291-review-integration.patch`. Section comparison with review 8 found
154 identical sections, 34 changed/new and none removed. The changed sections
were evidence/design material; executable content matched that review before the
new test below. Preserved the coherent existing changes with local checkpoint
`31d34f272cd06e5f13b77cec0019b95e2ee60670` before adding coverage.

Traced scoped conversation launch, shared PTY attachment, current-Wave filtering,
retained outer/inner layouts, native view/focus ownership, opening and both
direct/shell completion. Reviewed the projection, Task inspectors and appearance
boundary against the full design. No second lifecycle or workspace authority was
introduced. Fresh searches find one Podium caller per `query.sessions` and
`query.roadmap`, one root registry, and none of SessionScope, SessionContext,
SessionGroup, SessionRowItem, PodiumConsole, PodiumSurface or `_loadHierarchy`.

Scope-aware fresh conversation remains the latest accepted direction. It does
not claim the original one-current-conversation lifecycle is implemented.
Directive editing, LOO-284 shared action/display fields, bounded conversation
design, human-selected external work and full trial/budget obligations remain
full-Task scope. They are not supplied by this fixture or by Loopflow dogfood.

## Exact configured boundary

The signed disposable `/tmp/loo291-iteration8/Loopflow Proof.app` still matches
SHA-256 `75fdef9e4d35fed6deeb01c3995bddcd5c7e0c6e5b7f88ce733aff01c4ae1abe`;
signature verification passed. This review's fresh probe launched owned PID
73081, reported `AX trust false`, `active false` and `AXWindows -25211`, then
terminated that app before any UI action/provider launch. A subsequent process
check confirms absence. The process ancestry runs through Ghostty; the relevant
host permission is **System Settings → Privacy & Security → Accessibility →
Ghostty**. No permission was changed or bypassed, and no user client was moved
or resolved. Historical trusted probes are separate observations.

[Probe](configured-ui-evidence/iteration10/provider-probe.swift),
[receipt](configured-ui-evidence/iteration10/provider.log),
[binary/source and cleanup facts](configured-ui-evidence/iteration10/receipt.json).
Do not repeat this automation until the host's permission state changes. The
next configured trial should select a fresh owned Session row after navigating
away, verify its exact existing provider/terminal, then exercise two real checkout
groups and their companions. Preserve the prior completed Session as history;
never substitute a user-owned Session or a matching title.

## Focused native proof

Added `WorkspaceNavigationProofTests/sessionRowRestoresWorktree`. It mounts the
real workspace with two fixture shell-attached Sessions and four real Ghostty
`cat` PTYs. Actual row actions switch groups and focus the original shell. The
outer split control displays both groups; selecting a visible group does not
duplicate it. Hiding and restoring a slot preserves both inner layouts and all
four native surfaces. The original unfinished draft reaches its child afterward,
and every child must reply in addition to PTY echo. Any shared opening attempt
fails the query fixture rather than creating another provider.

An initial fixture lookup omitted the `row-` component of its Session ID; the
test stopped before navigation. Correcting the identifier produced the final
pass without production changes or weakened retention assertions:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 \
  --filter WorkspaceNavigationProofTests/sessionRowRestoresWorktree
```

One test passed, exit 0 ([receipt](configured-ui-evidence/iteration10/native.log)).
No Swift edits followed. This proves mounted native behavior with mocked shared
records, not live provider registration or external AX interaction. No broad
gate ran. Concurrent compression notes and iteration 9 review artifacts observed
after the checkpoint were left untouched and are not claimed by this review.

Under review-slice's condition, “When all applicable `Done when` claims hold,”
the configured gaps still preclude publication. No PR publication, Task
completion, landing, installed-app replacement or external-product trial occurred.
