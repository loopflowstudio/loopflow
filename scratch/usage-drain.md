# Codex usage drain — containment, 2026-11-13

Observed 03:48–03:50 Pacific (11:48–11:50 UTC). Success for tonight means no repeated restore/measure/revert cycle without usable GPU admission, while useful unrelated work continues. No source, graph, model preference, task assignment or external message was changed in this step. The confirmed loop had already been interrupted before this Session began; no second interruption was needed.

## Original report

The incoming incident report said: “Prior investigation reported 306 Codex sessions Nov 12 4pm–Nov 13 3:32am Pacific, mostly gpt-6-astra high; implement/compress dominated usage. MTG-338 allegedly repeated implement → measure → optimize-plan six times: restore eight candidate files, no L4 GPU capacity, revert, retry without new capacity. EXP-56 repeated implement/compress/realign nine times plus 19 routing sessions. These are prior findings to verify, not freshly verified facts.”

## Verified observations

`lf history list --all --limit 3000 --json` reached earlier than the requested window. Filtering AgentProcesses whose executable is `codex` and start timestamp is within 2026-11-13 00:00:00–11:32:00 UTC gives **306 processes, 306 distinct Sessions**. This counts starts, not all Sessions consuming tokens in that period.

`lf history show <session> --json` for those 306 Sessions yielded 615,009,183 total input tokens, including 593,270,528 cache-read tokens, plus 2,005,075 output tokens. Implement: 214,733,279 total input / 836,610 output; compress: 132,559,830 / 363,036. Together they account for 56.47% of total input and about 60% of output. These are lifetime receipts read around 03:50 for the start-window cohort, **not usage clipped to 03:32**, dollars, or subscription-quota measurements. Model is null in every history detail; the majority-model/high-effort claim remains unverified. Active turn snapshots do show gpt-6-astra. Raw counts and sanitized receipts are in `scratch/usage-drain-observations.json`.

### MTG-338: confirmed futile repetition, already stopped

Repository `/Users/jack/src/manabot`; checkout `/Users/jack/src/manabot.mtg-338-make-ppo-updates`. Status from the loopflow repository says “this Task is not registered in Loopflow”; status succeeds in its owning repository. This is lookup scope, not missing access.

Graph `62419322-28ee-4c58-afb0-0a4c19e33f14` ran optimize. Full recorded step list has 12 measure starts (one initial baseline, last interrupted), 11 implement and 11 optimize-plan. By the 03:32 cutoff there were six implement starts, seven measure starts and seven optimize-plan starts. The loop continued beyond that report.

History conclusions:

- `session_7234e56f94244e459d266953d7a6da62`: “Restored all eight candidate/harness files exactly from `02c202956`, preserving measurement evidence.” Checks: eight passed, eight CUDA skips; no capacity read or launch; full equivalence and L4 timing pending.
- `session_d85f69c3f5d54221a7a0ef46c25a60cf`: “L4 admission failed despite positive availability. No paired samples or CUDA correctness checks ran.” This corrects the blanket claim that every retry had no new capacity signal: at least one positive availability signal still failed admission. It restored the kept implementation, commit `5d7fc50d4`, and reported no rental or queued work.
- `session_1149d7af44664e249299cdf755252ddc`: “L4 capacity was unavailable at 11:43:03 UTC. No comparison launched; candidate timing and correctness remain untested.” Prior baseline 0.26437 optimizer steps/s, five runs, range 0.26366–0.26474. Reported restoring kept implementation and no rental/queue.
- `session_d1027a70090d402c825c9a75d033c2a9` returned `decision: iterate`, with “Pass 10 adds negative L4 availability evidence; candidate correctness and performance remain untested.” It again planned recovery of eight frozen paths. Parent command `12261b14-71af-4d10-bb4a-e27b70ea5b6d` supplied “new evidence counts as progress” and blocked only “when a person must resolve the question.” This is a concrete lead for debug/5whys, not yet a causal conclusion about the entire system.

Existing containment: history command `5059e725-9d63-4d3d-b3dc-dc3961afc33b`, `lf task interrupt MTG-338`, succeeded at **03:45:56–03:45:57 Pacific**, caller Session `35a1ee6b0d874093976b4f3d66541655`. Measurement `c6ad6704-6091-4793-ae6e-a2c66bc87550` exited 130 at 03:46. Two live `lf task status MTG-338 --json` reads confirm task idle, graph stopped at `measure · pass 11`, last updated 11:46:00 UTC. All its recorded headless Sessions are closed. `task interrupt` is documented to stop both the running Graph and scheduled automation. This step did not claim or repeat that earlier intervention.

The remaining failed check is candidate CUDA correctness and paired L4 timing. Usable GPU admission, not another negative availability observation, is needed to advance that experiment. Do not restart the unchanged optimize cycle as a way to check capacity. No rental probe was launched in this investigation.

### EXP-56: repeated reviews, completed delivery graph

Repository `/Users/jack/src/etude`; checkout `/Users/jack/src/etude.exp-56-give-a-training`. History confirms nine each of implement/compress/realign, 19 loop-or-next, eight concept-review, two QA, one publish (plus write-docs).

Graph `e70c6118-5d6c-46dc-ab0e-5e42868e6999` stopped at 01:53:30 UTC. Later graph `7eae9bcc-7249-455e-94e5-2206d9530da7` completed publishing at **03:12:28 Pacific**, PR https://github.com/loopflowstudio/etude/pull/103; task position is demo. Status comments record conversion to draft. No delivery graph is currently repeating. An active interactive Codex Session is titled “EXP-56 Usage investigation”; it was left alone.

The count alone does not prove nine futile passes. Finals `session_828c9e6db0d5476f9e5f252f7d656c8d` and `session_eb8dd998837e4c8c913d8e46feb8b5ef` describe concrete retry and plain-file-launch fixes, tests and advance decisions. Deeper review should separate new defects, scope expansion and avoidable repeated review. No interruption was justified here.

## Other live work and verification

`lf process list --json` and `lf session list --all --headless --on codex --limit 0 --json` inspected current runs. Process “waiting” labels include long-lived/unresolved records and do not establish token consumption. Closed Sessions can also appear in the Session inventory; filter state and turn, not presence alone.

The active headless Codex set in the snapshot was this investigation plus MTG-297 demo, MTG-329 implement, MTG-333 implement and MTG-335 implement. Recent `lf history show <session> --transcript` observations:

- MTG-329 `session_e46b2792b06b4d47abeba75dbf53bf18`: Etude caller and selection checks passed; working on missing numerical-error diagnostic capture. Useful ongoing changes, left running.
- MTG-333 `session_505d73fbe1164c319b8662f8b024b587`: backend injection, seeded-sampler equivalence and caller migration; current edits and focused tests. Large cumulative usage, but no confirmed futile repeated graph cycle; left running.
- MTG-335 `session_a2c571bc23ad4b2f96d6d050a7b8bae1`: real paired CPU/MPS benchmark; first pair completed in 16.9 minutes, next pair running. Waiting reflects live measurement, left running.
- MTG-297 moved from gate/publish to demo during inspection; left running.

Containment was verified through live lf task status and history, not a unit test or by restarting the expensive workflow. No source tests apply to this observation-only step. No servers, watchers, child agents or ongoing background jobs were started. Temporary read commands completed. No credentials were accessed.

## Handoff

Debug should inspect the optimize decision contract and why repeated capacity failure allowed another implementation recovery. 5whys should distinguish model decisions, graph routing, how the stop option is worded, and review expansion in EXP-56. Launch-plan should choose a bounded resumption condition based on usable capacity and decide whether prevention belongs in prompts, runtime budgeting or both. Global model downgrades and blanket interruption are not established remedies. The original quota drain is not proven resolved system-wide; the confirmed futile graph is stopped and unrelated active work remains intact.

A concurrent note `scratch/usage-direction.md` appeared during this step and was left intact. It records Jack Heart: “i want to be clear that i think this is someting that can happen in general with our loops, and we maybe need to impleemnt more stopping facilities”. Carry that broader scope into debug and launch-plan: examine shared Graph stopping facilities, iteration counts, usage/time controls, no-progress decisions and returning partial work; MTG-338 is an example, not the complete repair scope.
