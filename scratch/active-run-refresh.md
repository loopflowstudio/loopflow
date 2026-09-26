# Bounded active Run discovery and shared refresh

## Result: candidate rejected before publication

The environment-only locator is not a valid replacement for receipt discovery.
macOS KERN_PROCARGS2 returned status 0, argc 1 and only 29 bytes for an owned
`/bin/cat` launched with LF_RUN_DIR: that locator was absent. The same query on
an owned Python child returned the locator. [Redacted OS observations](active-run-refresh-evidence/os-locator.json)
contain only counts and the boolean comparison, never the environment itself.
The underlying reason for the different visibility was not established.

The existing Rust behavioral proof then failed both waiting-native-client cases:
the exact live Run disappeared, and capture-only observations reported missing
ownership. Three other tests passed. [Failure](active-run-refresh-evidence/rust-failed.log).
The proposed implementation additionally checked that malformed historical
receipts were irrelevant and inherited markers could not claim another client;
those assertions cannot rescue the missing legitimate client.

Three Swift candidate tests passed for shared refresh, error/missingness and
native draft/companion retention. [Candidate receipt](active-run-refresh-evidence/swift-candidate.log).
That does not satisfy the failed Rust boundary. Removed all seven candidate
source/test changes, including automatic refresh and its UI claim. The source
again matches starting checkpoint 510a48b44 for those files. No executable
change survives this pass, and no new proof of the restored source is claimed.
The rejected patch remains at `/tmp/loo291-rejected-environment-discovery.patch`;
its [hash receipt](active-run-refresh-evidence/candidate-sources.json) identifies
the failed candidate, not current source.

## Revised next implementation boundary

Keep the existing native receipt as authority. A cold one-shot read cannot both
avoid historical enumeration and discover older clients from this optional OS
locator. Do not fix this with an arbitrary recent-history window, a second index
written only by new clients, or a fallback that silently keeps the full scan.

The next bounded design should keep discovery alive across observations: perform
one explicit cold discovery of existing receipts, follow filesystem publication/
removal through an OS change feed, and revalidate cached candidates against the
same live PID/start evidence. That cache must be transient within the shared
reader, with lost notifications forcing an explicit rescan and an unavailable
observation until complete. Preserve one Mac reader across all panes; do not add
a durable Watch model, per-pane pollers or another process-ownership registry.
Its cold/warm costs and recovery path need proof before automatic Monitor refresh
is enabled. This is the proposed next architecture, not implemented behavior.

The implement skill requires: “If implementation reveals a counterexample that
invalidates the slice, authority model, deletion path, or full-design trajectory,
stop dependent work and revise the design or return to human review.” This
counterexample invalidates the chosen discovery path, so dependent polling was
removed rather than shipped with omitted clients.

The other writer's scrolling-during-refresh measurement now has 504/504
observations in `desktop-scroll-refresh.md`; that contribution was preserved.
No performance gain, installation, publication, PM mutation, provider transfer
or Task completion is established here. `git diff --check` passes.

## Rejected candidate design

Finish: Monitor observes newly live and exited Task Runs automatically through
one Podium reader. Native discovery cost follows the live process population,
not retained Run history. Existing clients remain discoverable without a restart.
Exact Work/Run attribution, PID/start checks and explicit gaps remain required.

The native launch already supplies LF_RUN_DIR. Read that one locator from the
live process's initial environment, then verify its existing Home-local Run
manifest and exact provider-client receipt. The environment is a lookup hint,
never ownership; inherited markers without a matching receipt cannot claim a
process. Remove active discovery's record_dirs traversal. Do not add an index,
dual writer, migration, recent-history cutoff or legacy fallback. Do not log
process environments; extract the locator in memory and discard the buffer.

Use macOS KERN_PROCARGS2 and Linux /proc/<pid>/environ, preserving spaces and
skipping argv before examining environment entries. Apple documents the layout
in its [ps implementation](https://github.com/apple-oss-distributions/adv_cmds/blob/main/ps/print.c);
Linux documents the initial environment and access restrictions in
[proc_pid_environ](https://man7.org/linux/man-pages/man5/proc_pid_environ.5.html).
Unavailable process evidence must remain explicit for provider candidates.

Proof: an old waiting client without capture bindings remains visible; another
Task in the same checkout never joins it; dead clients disappear; malformed
unrelated historical Run directories do not affect the observation; an inherited
locator without a receipt does not establish ownership. Then use the existing
window refresh loop and verify shared Monitor content updates from populated to
empty while preserving selection and existing terminal ownership.

Other writers' performance/scroll-refresh work is preserved. No configured
provider launch, migration, installation, external edit or publication is part
of this slice.
