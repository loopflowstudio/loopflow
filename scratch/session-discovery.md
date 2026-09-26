# Current-conversation discovery

The human expects the conversation they are using to appear under LOO-291.
Finish line: the shared Session list returns this exact provider conversation
with LOO-291's durable Work identity, and the demo reads its owning Home.
A row invented from a matching title or checkout, a second provider launch, or
moving the current client does not count.

Observed: Run run_643d86d70d5b4109a460e80a87a97533 is stored in the selected
installed development Home, not the main Home used by the preceding demo.
Its original manifest says headless and task:LOO-291; its native interactive
client receipt names PID 37189, matching the live Codex PID and birth time.
Provider history identity is 01a0d14a-ed43-72b0-9903-81b636fb139c.
The shared scanner excludes all original headless Runs. The Session attribution
parser accepts only internal Work IDs, dropping this declared issue selector.
These are separate defects from terminal attachment or provider liveness.

Correction: recognize interactive history through the existing provider-client
namespace, which is only created when an interactive child is published and is
retained after individual clients exit. Preserve original Run launch provenance.
Use the same rule for list and open. Explicit resolution still removes Sessions;
client exit only changes liveness. Resolve declared issue/slug selectors through
the existing Work-binding resolver, without guessing attribution from cwd.
Unresolvable subjects remain reachable as unmatched Sessions with a diagnostic.

The disposable demo must use this conversation's selected Home for both reads
and actions. Do not combine records from two registries while routing actions
to only one, or modify the installed app/global registry configuration. Other
unbound conversations and cross-Home presentation remain separate questions.

## Verification

The focused regression passes: original headless Runs are omitted; publishing
an interactive client includes one; client removal retains it; explicit resolution
removes it. Command: `cargo test -p loopflow --lib
unresolved_provider_scan_keeps_interactive_resumes_until_resolution`, exit 0,
`/tmp/loo291-resumed-session-test.log`. CLI build passes, and diff whitespace
check passes. No broad gate or commit performed.

Fresh configured reads from the conversation's Home return this exact Run as
active with Work task_2aa71a7e36fe416d8a721e2b2f7c54e7, equal to LOO-291's
roadmap runtime Work ID. The native command names the matching built CLI through
LF_BIN. Receipts: `/tmp/loo291-active/resumed-{sessions,roadmap}.json`.
Original Codex PID 37189 and birth time remain unchanged after the demo relaunch.
No Session open/transfer/complete or provider restart was performed.

Demo PID 41489 replaces the owned PID 44263, same disposable signed Swift app;
its CLI now includes the shared discovery correction. Reads and actions use the
same selected development Home as this conversation. Launch receipt:
`/tmp/loo291-active/resumed-launch.log`. Human visual confirmation is pending.
