# Execution and capture naming

LOO-298 · Jack Heart · 2026-09-30 · Item 4.

Jack requested a separate naming commit after the landing fixture repair
`f02565d8d`. This cut changes names and the public Flow graph's provenance key;
existing SQL, captured graph encodings, manifest bytes and execution ownership
stay intact. No history-table rename is selected.

| Before | After | Meaning |
| --- | --- | --- |
| `RunManifest` | `SessionCaptureManifest` | One captured input's manifest; a Session can retain several and an import need not establish an Exec. |
| `RunSpec` | `SessionCaptureSpec` | Inputs for recording a capture. |
| `RunCapture`, `RunRecorder` | `SessionCapture`, `SessionRecorder` | Capture-local recording into conversation history. Neither becomes another lifecycle owner. |
| `RunContextRef`, `RunContextArtifact` | `SessionContextRef`, `SessionContextArtifact` | Immutable prompt evidence. |
| `RunEvent`, `RunEvidence` | `CaptureEvent`, `CaptureEvidence` | Historical artifact events and their reduction; distinct from native Session events. |
| `RunLaunchRequest` | `AgentExecRequest` | Replayable provider inputs used by an Exec; no request table or lifecycle. |
| `RunFlowStep`, `RunFlowMembership` | `SessionFlowStep`, `SessionFlowMembership` | Captured conversation membership; missing historical membership remains unknown. |
| `RunWork` | `SessionWork` | Typed Work supplied at conversation admission. |
| `RunAttribution`, `run_attribution` | `ExecAttribution`, `exec_attribution` | Process attribution for the command journal. |
| `run_record` module | `session_record` | Capture artifacts and conversation evidence. |
| Manifest Rust fields `run_id`, `parent_run_id`, `launch` | `artifact_key`, `caller_artifact_key`, `exec` | Historical selector, caller evidence and provider request. Serde retains the original on-disk keys exactly. |
| `CaptureHandle.run_id()` | `artifact_key()` | Artifact selection, not process or conversation identity. |
| `LaunchPromptInput`, `PreparedLaunchPrompt`, `prepare_launch_prompt` | `ExecPromptInput`, `PreparedExecPrompt`, `prepare_exec_prompt` | Shared provider preparation. |
| `LaunchResult` | `AgentExecResult` | Provider result observed by its Exec, distinct from the outer command outcome. |
| `LaunchTarget`, `SessionLaunch`, `TaskWorkerLaunch`, `TaskLaunchOptions`, `StepLaunch` | `ExecTarget`, `SessionExec`, `TaskWorkerExec`, `TaskExecOptions`, `StepExec` | Existing process entry inputs/results, without new product objects. |
| `launch_agent`, skill/Session/worker/driver launch helpers | `exec_agent`, corresponding `exec_*` helpers | Execute through the existing shared process path. |
| Review/Ask `*_run` capture/client helpers | Capture, Session and Exec names matching their actual owner | Reservation/publication use captured events; native client control retains exact process authority. Historical environment keys remain unchanged. |
| `launch_user_name` | `participant_name` | One participant resolver, also used outside process startup. |
| `expand_flow`, branch/step/chain helpers | `compile_flow`, `compile_branch`, `compile_steps`, `compile_with_sources` | Compile definitions into the saved graph before execution. |
| Compiled step `flow_parents`, public graph `parents` | `sources` | Definition provenance, outermost first. Rust/Swift/current DTO fixtures change together. |

## Why definition provenance remains

`TaskFlowView` displays `from …` using the graph's definition chain. The compiled
Skill also uses it for its display path; prompt-log and commit formatting retain
their source labels. These are real display consumers, so Jack's conditional
delete rule does not apply. They now say `sources`. The saved graph codec still
writes `flow_parents`: preserving that historical format keeps immutable captures,
import equality and saved source-independent execution unchanged. The public
`FlowNode` wire uses only `sources`, without a compatibility alias or default.
Filesystem ancestry, Git ancestry and OS `launchd` terminology retain their
ordinary meanings. Historical manifest selectors, environment/artifact encodings,
and the command journal's trace names remain readable evidence.

## Proposals only, for Jack

| Current table | Candidate | Status |
| --- | --- | --- |
| `session_events` | `agent_events` | Jack's tentative suggestion; not applied. |
| `run_events` | `exec_events` | Candidate for the Exec-side command journal; not applied. Its key remains a trace, not a process per event. |
| `flow_events` | `flow_events` | Already names its owning Session. |

No CLI command is renamed. Current docs use the shortest resolving commands;
`sources` is the sole renamed public JSON field. Old saved plans and manifests
retain their existing encoding, so this cut requires no migration or dual writer.

Proof results and production measurement belong in the existing
[evidence ledger](evidence.md). The remaining import and configured acceptance
obligations are unchanged.
