# v0.14.5

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.5 repairs two interruptions to ongoing work: repositories stranded by the 0.14.4 upgrade and OpenCode diagnostics covering unfinished terminal input. Existing Task checkouts can restore their retained repository identity through ordinary reads, while OpenCode errors remain available in a private log without obscuring the composer.

## Keep existing Tasks usable after upgrading

The repository relocation support in 0.14.4 could leave older repositories without the Git locator needed to recognize their retained identity. Ordinary reads now enroll that identity when a retained local Task checkout still matches the Git common directory and branch, avoiding manual binding when that evidence is available. (#1583)

- Keep existing repository identity and Task work connected, including through symlink aliases and concurrent reads.
- Relocate populated planning destinations and their routes in one transaction without premature foreign-key failures.
- Preserve explicit recovery when evidence is missing or ambiguous; repository names and remote URLs do not establish identity.

## Keep typing through OpenCode retries

Raw OpenCode errors could overwrite the terminal composer during errors and retries. Launch and resume now capture stderr separately, leaving OpenCode in control of its native error display and keeping unfinished input readable. (#1582)

- Find full diagnostics in the announced `opencode.stderr.log`, opened with owner-only permissions. Capture preserves arbitrary bytes and subprocess diagnostics.
- Resume with the supplied native Session identity intact.
- Keep the provider running if a log write fails: capture continues draining stderr and reports the failure after the provider exits.

## Operational notes

- The OpenCode change fixes diagnostic overlays, not model-access denial. An installed OpenCode 1.19.3 probe with synthetic local responses reproduced three overlays on lf 0.14.4 and none on the candidate, with all three errors retained in the log. Paid-account access, OpenCode 1.18.33 and approval UI were not verified in that PR. (#1582)
- A new CI installation proof starts with checksum-verified lf 0.14.3 and a real Task checkout and Session. The recorded run verified identity, historical Process paths, unpublished edits, CLI and Desktop destination reads, a later directory move and promotion retry. Configured macOS installation and real native-provider acceptance remained outstanding at implementation delivery. (#1583)