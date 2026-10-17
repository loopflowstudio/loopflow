# LOO-415 assumptions

October 17, 2026: use a fixed five-second acknowledgement deadline after connecting
to the local owner socket. This bounds the identified silent-owner wait without a
new configuration knob; it does not bound SSH startup or an established attachment.
Cancellation uses the existing CLI signal handler and closes only the viewer.

October 17 gate: interpret independent ownership as applying to direct conversations,
Flows, Task execution and shells. A release, PR or sync controller retains its
existing cancellation lifetime; any conversation it launches owns its transport
separately. Using `may_launch_agent` to wrap those controllers changed release
recovery and checkout cleanup in the affected suites, beyond the Session-continuity
outcome. Explicit EOF from a controlling pipe viewer closes command stdin after
accepted bytes drain; socket loss alone never sends EOF.
