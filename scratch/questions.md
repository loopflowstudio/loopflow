
## Coordination CPU, 2026-11-09

Step 2 assumption: fast-forward the clean tracked branch from `646b20fb24` to the locally available `origin/main` (`4848816f73`) in this checkout. The old tree predates TaskUpdates and the sampled config path; all scratch files were hashed before the fast-forward and verified unchanged afterward. No installed executable or running process was replaced.

Step 2 choice: Git tracking/ignore protection runs when LF writes private settings. Reads use the file as it stands, including a tracked file, and do not run that diagnostic. This removes subprocess work without caching stale settings; the writer still refuses to change a tracked private file.

Assumption: temporarily lowering scheduling priority for exact installed Codex observers and asynchronous Task-update workers is preferable to restarting conversations or changing provider/tool priorities. Used reversible `taskpolicy -b`; metadata can lag under contention. No observer restart or provider input is authorized by this assumption. Roll back with `taskpolicy -B` after rechecking each saved PID/birth/command identity.

## Auto implementation, 2026-11-09

Measured unrelated-write amplification justifies one additional derived revision domain in the existing store, rather than a new watcher/cache architecture. It is deliberately shared across Task observations; reportable activity on other Tasks can still invalidate readers. Preserve live/pending/error fallback behavior.

Context constraint: the seven research Markdown files that must be preserved total 117,852 bytes, already exceeding the 98,304-byte scratch target before incident/open-work notes. Their contents and the parent HTML stay untouched. `lf context` before this note reports 32,921/12,000 tokens and 147,141/98,304 bytes. The limits remain unchanged; this preservation conflict prevents meeting both targets in this implementation step.

After implementation notes and concurrent parent research, `lf context` reported 37,412/12,000 tokens and 166,237/98,304 bytes (over by 25,412 tokens and 67,933 bytes). Research, including the parent's new performance comparison, remains untouched.

## Observer baseline, 2026-11-09

Scope assumption: this run is the optimize baseline step described in its message, not a second invocation of the enclosing optimize Graph. Keep checkpoint `4d7de3695c9791fcc24164557c41f461703f29a4`; change only opt-in test measurement code and results.

Baseline claim to test: “At checkpoint 4d7de3695c, settled native and Codex observation costs X CPU ms and Y SQLite VM steps per observer poll; unrelated Session activity does not increase that work.” One versus sixteen logical observers share an isolated database containing 32 completed Graphs. Native startup has no Task binding, avoiding remote Task sync; bindings are introduced and delivered before timing. A sleeping fixture client and local WebSocket provider replace live providers. No real provider input or installed process changes.

## Subscription experiment, 2026-11-09

Assumption: start with reuse of the existing server runtime as the simplest
candidate for repeated subscription setup/teardown. Establish a separate frozen
1/16/64-client baseline first, including duplicate projection counts; do not
infer server cost from the settled observer results. The candidate thresholds
in `coordination-subscription-design.md` are experiment decisions, not an
established production CPU budget. If runtime lifecycle is not material,
return to planning with the measured limiter before editing production.

## Shared subscription implementation, 2026-11-09

Use exact serialized Read/select plus database path and current directory for
server demand identity. The server has one fixed process environment; no
request-specific environment mutation exists on this read path. Internal
monotonic demand generations are separate from client checkpoint values.

Context preservation conflict: the supplied instruction preserves existing
research and measurement/review artifacts, backed up at
`/tmp/loopflow-coordination-artifacts-before-sharing`. Initial `lf context`:
50,496/12,000 tokens and 223,936/98,304 bytes. Retain those artifacts and limits;
curation cannot meet the targets while retaining their required content.

A concurrent checkpoint, `1a1caa37d7fe9864677d954924a7befb926a31a1`, has the
subject “lf pr open: prepare branch” and captured in-progress source plus scratch.
No attribution is inferred from its Git identity. Keep that history. Separate
source-only commits under `refs/loopflow/checkpoints/` isolate the TaskUpdates
repair and complete sharing change without moving any branch or checkout; the
active branch's final source matches them. Comparison instructions distinguish
that review stack from the active history and debug smoke from optimized timing.

After the handoff notes, `lf context` reported 52,231/12,000 tokens and
231,650/98,304 bytes (over by 40,231 tokens and 133,346 bytes). Required research
and measurement preservation still prevents meeting both limits. The final
query is saved at `/tmp/subscription-sharing-20261109/context-final.txt`.
