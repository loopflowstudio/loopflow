# Session architecture direction — October 16, 2026

Jack Heart requested the Rex comparison, shell persistence and lfdb/provider
continuity study, then accepted per-Session ownership without a shared server.
The [research](session-ownership-research.md) retains findings and evidence.
Jack next requested historical failure analysis (retained at
`a3e929a7ef:scratch/session-continuity-failures.md`),
then [prefetch and caller-death analysis](evidence/session-readiness-before-stack.txt), explicitly covering
nested CLI and basic Session creation independent of Desktop. Jack also wants a
pleasant fresh-context restart after reliability is established. The
[original direction record](evidence/rex-research-direction-full.txt) retains the
sequence; initial provisional status is superseded by later acceptance.
