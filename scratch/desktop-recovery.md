# Desktop session recovery

2026-11-08 report, Jack Heart:

> (a) sessions that hit usage limits didnt auto transfer to codex accounts with space, despite recent diff attempting ot help with this
> (b) one session (718)  Is stuck with Resume paused goal? and not accepting input
> (c) some sessions just wont open
> (4) there's no "refresh" button as there should be for sessions to deal with situations like this

Observed here: Desktop's LOO-718 pane shows Codex's paused-goal dialog. Its
provider and transport processes are alive. Sending Escape through Desktop's
pane API returned success but did not dismiss the dialog. No goal was resumed.
MTG-252's screenshot names Claude thread e80bd648-0697-4548-b0a4-4a82710fdae8;
that transcript was absent from both native and stored Claude homes when checked.
Desktop logs repeatedly show `select a repository before reading execution location`.

Further observations on 2026-11-08:
- LOO-718's viewer-only reconnect did not restore input. Escape advanced the
  native Codex tty's byte offset by five bytes without dismissing the dialog.
  Stopping idle Codex PID 21650, reopening the same saved conversation, and
  sending Escape dismissed it. The goal stayed paused. The cause inside that
  Codex client remains unknown; this was recovery, not a cause fix.
- The ended relay showed “Press any key to close the terminal” while Desktop's
  connection-input guard rejected those keys.
- MTG-252 already had Codex Session session_99ffa20715cd497fada9abfce92cbaee.
  Its opening messages included “reusme the claude session that was hapening in
  this workspace” and “we were testing depth 4 attetnion with different critic
  strategies”. Bound it as primary, replacing the missing Claude reference.
  `session resume --prepare` resolves Codex and reports Reconnect; the installed
  Desktop still reports no surface for this Task.
- The recent quota change automates Graph fallback. Native Claude's limit hook
  only switches Claude logins; it has no Codex fallback. Native Codex observes
  usage but does not perform that recovery either.

Work remaining:
- Extend interactive quota recovery to eligible accounts/providers. A pending
  question asks whether switching should continue the interrupted work or wait
  for a new message; neither behavior has been implemented here yet.
- Verify MTG-252 opens in Desktop after the repository-location fix is installed.
- Verify the Refresh button and ended-terminal key handling in a built app.
- Trace the native Codex input hang if it recurs; reattachment did not fix it.

Checks: focused Rust location regression passed; 69 Swift tests across five
suites passed; Swift rebuilt after ended-terminal repair; cargo fmt and
`cargo clippy --all-targets -- -D warnings` passed. Native terminal interaction
still needs the built-app check above.

## All-session recovery, 2026-11-08 evening

Jack Heart:

> 696 is still unresponsive. can you look into all my open sessions and fix thtem all

A steer relayed a screenshot of MTG-261: “Conversation failed to open” with
“Error: select a repository before reading execution location”, Retry,
“Recorded checkout is missing / Restore checkout” and “Reading workflow…”.
This is the relayed screenshot description, not a quotation from Jack.

Initial inventory: two Desktop windows, 13 native Session surfaces (including
retained hidden workspaces); `session list --all --limit 0` returned 530 rows:
15 active, 59 detached, 13 unknown, 443 closed. The current parent
`session_e52c5b3724d6486d901c84e643e09005` and this unbreak run
`session_c599afa6462f4976b2cf7f7d4b34a0af` are protected throughout.
Raw local observations are under `/tmp/loo813-recovery/`; they include the initial
Desktop state, all Session records, process identities, pane captures and
read-only transport snapshots. No diagnostic message was submitted.

Observed before further recovery:
- LOO-696: Desktop capture initially showed an empty prompt. Inserting `x`
  revealed `whats wsasx`; Backspace left `whats wsas`. Desktop focus succeeded.
  The native editor responds, and its existing draft remains. No restart.
- MTG-269 exhibited the same stale empty prompt: insertion revealed
  `gowgows itasfokaasfx`; Backspace left `gowgows itasfokaasf`. No restart.
- LOO-718, engbot, list, spend and study accepted and removed one character
  through the Desktop API. LOO-718's goal remains paused at its usage limit.
- MTG-175 displayed an active Claude turn and draft `asd`; neither was touched.
  MTG-252, MTG-267 and training-throughput b015d5 displayed working turns.
- Infrastructure and engine-performance displayed Codex sign-in screens.
  Both used the `engineering` account, whose cached credential state was
  `missing`. Their original Codex JSONL histories were readable, 17 records each,
  ending in `task_complete`. Reopening with the connected account keeps the same
  provider thread; Desktop reconnection is still being checked.
- MTG-261's recorded checkout `/Users/jack/src/manabot.0253da-find-out-why`
  was absent. Ordinary `lf task checkout MTG-261` failed because remote branch
  `jack-heart/0253da-find-out-why` was gone. GitHub PR 432 records merged head
  `1841412e42dd667ad6860554754a8134294e9010`, merge
  `c59aeee0ede2b5d9c05a8ad3ce61f89134e60287`, merged 2026-11-06T04:09:59Z.
  `lf task checkout MTG-261 --recover-from 1841412e42dd667ad6860554754a8134294e9010`
  restored the existing Task checkout. No new Task was created.

The first read-only transport snapshots of detached clients showed strings such
as `65535u5;1u` and `65535u7;1u` in Claude and Codex prompt rows. No input had
been sent to those clients by this run. The observer sends only `attach` with
`control:false`, `replace:false`, `own:null`, then `leave`; it sends no key or
terminal-response frames. These are reconstructed snapshots using pyte, so a
parser artifact remains possible until checked against Desktop captures.
No existing draft was erased. Preserve this keyboard-protocol clue for debug.

## Recovery result

- No goal resumed, no diagnostic message submitted, no blanket process stop.
  Only the three idle sign-in clients were reopened with an explicitly selected
  connected Codex account after checking their saved native histories. Original
  provider thread IDs stayed the same. The authentication errors visible above
  their restored prompts are saved conversation output; fresh weekly-capacity
  readings appeared. No new model turn was submitted to test authentication.
- MTG-261 now has its original checkout restored and opens through production
  Desktop. Opening provisioned its previously absent interactive primary,
  `ee411e2f7d2c415eb83899367441b93a`; it reported the merged PR and idle workflow.
  Earlier MTG-261 Sessions were all headless. The opening did not run its Graph.
- Built and installed `~/Applications/Loopflow Dev.app` through
  `uv run python scripts/loopflow-dev.py install` and launched with `run`.
  Production `/Applications/Loopflow.app` was not replaced. Dev remains open as
  the temporary working view for LOO-434 and engine-performance. Their production
  panes were not proven recovered; engine-performance still showed an ended relay.
- Added `lf desktop do refresh-session -p session` to the local Desktop action
  registry, using the existing Refresh implementation. This was needed because
  CUA returned `cgWindowNotFound`, and the installed API cannot invoke Refresh.
  Dev's Refresh reopened the existing conversation; its insert/delete checks
  passed. The production end-of-relay key-dismissal fix is still not installed.
- Refresh changes the viewer, not the native client. LOO-696's and MTG-269's
  empty-looking captures were stale relative to their drafts. API input worked;
  physical keyboard input cannot be verified in this headless run.

Checks: `swift test --filter 'SessionsStoreTests|DesktopPaneControlTests'`:
62 passed. The first run failed only because the new action was absent from
`desktop_action.json`; the fixture was updated and the same suites passed.
Swift app build/install passed. No Rust source changed in this pass; the earlier
Rust format/clippy results remain recorded above. Nothing published or merged.

### Remaining failures for debug

1. Production engine-performance still displays an ended relay. Dev opens the
   same thread and accepts input. Production LOO-434's switch receipt succeeded,
   but only Dev's recovered input was verified.
2. Quota-blocked native clients remain on their original account when a native
   draft or pending state cannot be ruled out. LOO-672's production pane showed
   the zero-capacity account and an unsubmitted `/logo` draft on the later check;
   it was not replaced. Automatic interactive account/provider recovery remains
   incomplete. Claude's two configured accounts were cooling down.
3. Older native processes remain alive without an owner socket. The observer's
   exact failed check was `connect(<owner>.sock): ENOENT`; these may contain
   unrecorded drafts or requests. They need native-state inspection before safe
   replacement, not blanket pruning. The per-Session inventory names each one.
4. The last explicit exact-Session openings of LOO-747, MTG-243, MTG-244 and
   MTG-245 returned opening receipts, then `desktop capture` reported “has no
   workspace in this Desktop window”. LOO-793 reported “no native surface”.
   These were bounded checks, not evidence that opening succeeded. Their passive
   native snapshots showed quota history; no input or restart was performed.
5. The relayed keyboard-protocol clue matches `RESET_MODES` in
   `transport/terminal.rs` and keyboard restoration in Ghostty patch 0003.
   This is a hypothesis for debug. Snapshot parser artifacts have not yet been
   ruled out; no draft cleanup was attempted.

All temporary CLI viewers were detached with Ctrl+]. No watcher or build remains
running. The parent Session and this run's current native identities were checked
again at the end; no process belonging to either was stopped.

### Complete initial open-interactive inventory

This is the full 74-row active/detached inventory across all repositories, not
just Desktop's visible panes. “Passive” means a read-only transport snapshot was
read; it does not claim an input or model-response check. Missing sockets are
reported as unverified, not healthy. Closed history was not reopened.

Counts: active-unmodified 6, blocked-account 7, checked-input 7, missing-relay 20, passive 23, quota 8, recovered 3.

| Session | Initial state | Check and disposition |
|---|---|---|
| 30df0c Embed fact and effect ids through the card encoder's exact vocabulary ins (`5c46a82e3afb49b0a92e9df5ad26e71e`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| Active work status (`f505abc0e07647b58fdf9d33c5017c25`) | detached | Unverified: owner socket absent; recorded native PID 82207. No replacement: draft/turn unknown. |
| Agent platform craft (`session_21feb1903a7c4c8f87a8ded394421dbe`) | active | Rex client: read-only snapshot; paused-goal status retained. |
| Codex session connections (`session_b68a12875148418f8361de1f2e5b8387`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| Competitor architecture research (`session_bc1dd22044004fd5a5cbbe748e3e5a20`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| Definitions as code (`session_3c7a3f1076944badb32d253c544b07e4`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| EXP-28 Combined eval database (`b3ede790bbb442febb9a72d74f599cff`) | detached | Unverified: owner socket absent; recorded native PID 30937. No replacement: draft/turn unknown. |
| EXP-40 Checkpoint wall time (`02f4ffa825c1457daf114b93bc60c27f`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| EXP-42 eval live progress (`274056d18b1b4ecbbd8ee1379b65a6a6`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| Engbot Wave (`617d2a8eaa7b40b5880cc610f4ea098b`) | active | Checked: Desktop insert/delete passed. |
| Fantasia repository operations (`ea5bcb72bd404139a2704faa083edd40`) | detached | Unverified: owner socket absent; recorded native PID 50794. No replacement: draft/turn unknown. |
| HumanLayer study (`session_ff1b9e68cd2c4055b8b8bdf42c0a82d2`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| LOO-312 Newcomer setup (`d566a570e17f465cb266110ae85f3a23`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| LOO-434 workflow experience (`01f484c818004d53b393cd8d59c2d090`) | detached | Recovered in Dev: same thread/account move, Refresh, insert/delete passed. Production reconnection not verified. |
| LOO-498 Open and restore (`session_d08da070c64442f68dacef019a3c9c86`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-498 Rex workspace (`310fb7b6015843e498316cd921a348b9`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| LOO-558 stage handover (`eb1f9cfd9a5045a79b16e262de519f7f`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| LOO-669 Revamp the release process: minors from main, release streams, lessons f (`f090e9dcf80641afb8d1a0f486979871`) | detached | Unverified: owner socket absent; recorded native PID 66000. No replacement: draft/turn unknown. |
| LOO-672 ready conversations (`a66598baebd440cd83b6684339920e95`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| LOO-696 Task concepts (`87d0876bb32941b8a97f81e44872c76f`) | active | Checked: Desktop insert/delete passed; draft `whats wsas` retained; no native restart. |
| LOO-718 Mobile architecture (`session_71e26e01ca6f41a1a9a6f1b4469b3813`) | active | Checked: Desktop insert/delete passed; goal still paused. |
| LOO-723 coherent installation (`322bfd3c1877409d9eea5205d3cd1e14`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-732 telemetry explore (`f24f2497f3fe4753a8a6d23cbcf84e99`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-733 review surface (`942a90ad9a5347d897dbe22c0922e8da`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| LOO-747 Ship recovery (`03d6e18c7c6146c2a0e3938cd6baba96`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| LOO-747 default (`session_fc498228861845a39262491f0706d78a`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-772 Task conversation (`session_8ebcd369c5e9479fbfe989fea0f7b65d`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-775 Watch recovery (`6c0210c03a964af1bc8cc31cd6fd8fd6`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-776 Keep keystrokes a viewer sent just before it disconnected (`99cbee9948054bdd8795f38b184075b1`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-788 Demo: measure a Task conversation launch (`fcb7642a89c34c4292eb03d080cd5ca5`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-793 Reply references (`85c52002fe524d5daeb4ee4a8e7f8102`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-810 creation recovery (`f53a9069597341d497c56a105eab2e3f`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-811 Recovery review (`e47ba8ae55044365af58d39fba6c2f83`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-813 Desktop recovery (`session_e52c5b3724d6486d901c84e643e09005`) | active | Protected parent: native process alive; no input or restart. |
| Land resume fixes (`session_cd7e1aab97e8425da06951bb1271c65c`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| List Wave (`6cf8740c22eb4236b5f27cbf90ece384`) | active | Checked: Desktop insert/delete passed. |
| MTG-175 Live Eval Reports (`f294041cf95f4c62bdb6b64ac8e56d33`) | active | Active/unmodified: Claude working, draft `asd`, account marked blocked. |
| MTG-212 Benchmark MTG-183's best checkpoint against the Jev decision model over (`636b009eccd244db836492c431b0ba4e`) | detached | Unverified: owner socket absent; recorded native PID 64105. No replacement: draft/turn unknown. |
| MTG-219 TLA Card Authoring (`d9c467555ed34b169e20acaf67694088`) | detached | Unverified: owner socket absent; recorded native PID 43702. No replacement: draft/turn unknown. |
| MTG-220 Decklist import (`1fb1694142e3486e9ddad9e40b945733`) | detached | Unverified: owner socket absent; recorded native PID 78926. No replacement: draft/turn unknown. |
| MTG-221 TLA T2: Casting, costs, zone permissions and private zones (`a17891fe2e26441382555d7332b5c6ad`) | detached | Unverified: owner socket absent; recorded native PID 72522. No replacement: draft/turn unknown. |
| MTG-222 TLA T3 Characteristics (`session_89ffcdf8d2444e88a9ae3249095c0609`) | detached | Unverified: owner socket absent; recorded native PID 80851. No replacement: draft/turn unknown. |
| MTG-222 TLA T3: Characteristics, counters, replacements and attachments (`b239e5500dfb4c9190c126484537559c`) | detached | Unverified: owner socket absent; recorded native PID 88006. No replacement: draft/turn unknown. |
| MTG-223 TLA T4: Triggers, history and stack behavior (`af0e58a294e547df806225111e9b3170`) | detached | Unverified: owner socket absent; recorded native PID 29585. No replacement: draft/turn unknown. |
| MTG-224 TLA T5: Permanent copying and linked-ability primitives (`e76c4e99e66a4cbcb9935767c661b97b`) | detached | Unverified: owner socket absent; recorded native PID 53356. No replacement: draft/turn unknown. |
| MTG-225 T6 Engine Foundations (`5d552358ea0940a1beca11c09a0110ab`) | detached | Unverified: owner socket absent; recorded native PID 40286. No replacement: draft/turn unknown. |
| MTG-233 Cull everything the codebase no longer needs (`fa6178b2144c47339d9db37f19bf1068`) | detached | Unverified: owner socket absent; recorded native PID 47023. No replacement: draft/turn unknown. |
| MTG-235 Effect Vocabulary (`81c33e7abbfc4484bf7d81f103db4501`) | detached | Unverified: owner socket absent; recorded native PID 41897. No replacement: draft/turn unknown. |
| MTG-236 Derived state revisions (`e5d9d2b388ee43349d8b7f3e249c107a`) | detached | Unverified: owner socket absent; recorded native PID 38711. No replacement: draft/turn unknown. |
| MTG-237 Engine budget review (`7f20038b9d4f48c9b3f9952a0ac44756`) | detached | Unverified: owner socket absent; recorded native PID 39371. No replacement: draft/turn unknown. |
| MTG-238 Catalog scale types (`eac235f5c3ed403eb9b7f3a94d3a3e75`) | detached | Unverified: owner socket absent; recorded native PID 37552. No replacement: draft/turn unknown. |
| MTG-239 Five Card Completion (`e2a4b3267d404a61885a763832bfbea5`) | detached | Unverified: owner socket absent; recorded native PID 68037. No replacement: draft/turn unknown. |
| MTG-241 Rules execution (`7e0bc5c92dd54c99b67e2cdea689197e`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| MTG-242 Mana Payment Resources (`e30ecfae91b84e97b2049b9d74318439`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| MTG-243 Catalog naming choices (`bbac3df212f24f2db8154ac1538f488e`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| MTG-244 Composable Layer Evaluation (`cac2909324d54d76bc1e28099e30245b`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| MTG-245 Growing Pool Protocol (`ebabbc8406ea42d5a524640af9789582`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| MTG-249 Representation experiments (`39a4fa719f2e4399880838fa94905c98`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| MTG-250 Sparse attention comparison (`312884d8ee3d4eaaa25b8b84719aca81`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| MTG-251 Separate learning seats from batch closure; compare closure and lambda (`7bc9684e2cba48e49a703732b9d4212c`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| MTG-252 Resume Claude session (`session_99ffa20715cd497fada9abfce92cbaee`) | active | Active/unmodified: production Desktop working turn; opens successfully. |
| MTG-267 Frontier self-play (`8f977fd226214c7eb489e6fb6ac6a390`) | active | Active/unmodified: production Desktop working turn. |
| MTG-269 Magic Mirror search (`session_c73fe3a8e9e94b56b1ffe915b575a9c0`) | active | Checked: Desktop insert/delete passed; draft `gowgows itasfokaasf` retained. |
| Release hang fix (`session_4386223ba09a4f0e8bdb1df40c445963`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| Repository operator (`session_4ff317503562462cb15dd4ce15b59cf0`) | detached | Unverified: owner socket absent; recorded native PID 71643. No replacement: draft/turn unknown. |
| Spend Wave (`c76153e219f549b5a5aa5fbede5926f0`) | active | Checked: Desktop insert/delete passed. |
| Study status (`f5acdee3953a449ea8529d905bbf2d3e`) | active | Checked: Desktop insert/delete passed. |
| b015d5 Training throughput (`881aafe7262040e486a79a1c429dfe16`) | active | Active/unmodified: production Desktop working turn. |
| default (`session_136d7b61eafc4c1a8137ddabb2941051`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| default (`session_aedeca37725c4897ad28414ed780af58`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| default (`session_e42c8f360cc64026ab0003f998c304dd`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| default (`session_f90a21bc15dd44dfb6c4fa67f2d100de`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| engine-performance (`5e99643c30514c3b95e78063030848c7`) | active | Recovered in Dev: same thread/account move, Refresh, insert/delete passed. Production pane remains ended. |
| infrastructure (`09a40b4c78b2411ab30bfbc3203d7304`) | active | Recovered: same thread, connected account; production Desktop input check passed. |

### Historical records without a known active client

The remaining 13 non-closed rows had state `unknown` and no recorded AgentProcess.
They are not established open native clients; none was launched or reset.

- Define durable subwave identity and memory addresses: `task_f8a2b8980fb74eec9c66fad1b590a7bf:08972f20-efde-4113-bfc5-e14b649ea195:task-design:review_kickoff:0`.
- LOO-572 Record one usage row per turn, stop recording internal commands, close d: `6df92ca4d9b249bfa6c379dd98e69735`.
- MTG-10 Define viewer-safe Study artifact and evidence contract: `f2ab8b42b2ce46f5b2c4d5b10fed54dc`.
- MTG-268 Batch local-update rollouts into one network call per step: `6c1c2159e220471e97887187d01d49eb`.
- Make Linear OAuth recovery survive unattended Project runs: `task_81bb835a6bd04748bf99e906aa9f4236:migrated-task-task_81bb835a6bd04748bf99e906aa9f4236-1790208246:ship-demo:review_demo:0`.
- Restore end-to-end Project and Task controller execution after rewrites: `task_f4f230f6de97470fa25be460bb9552ec:migrated-task-task_f4f230f6de97470fa25be460bb9552ec-1788413693:ship-demo:review_demo:0`.
- claude 27120774: `session_fcaecb6c626e46c182a2803262bc7b62`.
- demo: `session_3211bb1b212942d2909c0ca84b7b3880`.
- demo: `session_63fc75b803664b3db1bacc9c6bb0ad23`.
- pr-review: `session_06bcfcf41cc041aa9c1a9b64382a0f9a`.
- pr-review: `session_4fd10ead97ad4fd6819fcdc2e5437e6b`.
- pr-review: `session_6c312e9dbf144e84b30262524c953122`.
- repo: `142f167d3ba14d8e80e6c0240a702f85`.

### Final sweep

The final sweep covered both production windows and both local Dev windows,
including 25 retained Session surface occurrences. Six additional active Sessions
appeared during the pass (some were warmed by opening Desktop); all are listed
below. Existing active processes and drafts were left intact.

| Session | Final observation |
|---|---|
| LOO-800 launch checks (`a1f6ee01550d42cea338c291d1231b00`) | Desktop shows prompt; passive check only, unmodified. |
| LOO-809 Attach tickets for remote clients (`aff39379845f44b8a64b682a75705da9`) | Desktop shows prompt; passive check only, unmodified. |
| LOO-812 Session ownership (`a4f6654f7c0f43f4b5273601f739549d`) | Desktop shows prompt; passive check only, unmodified. |
| MTG-260 Elemental comparisons (`8eeadf4c3bae40448241e4b23dfa0335`) | Desktop shows prompt; passive check only, unmodified. |
| MTG-261 Development speedups (`ee411e2f7d2c415eb83899367441b93a`) | Desktop shows prompt; passive check only, unmodified. |
| MTG-268 Batched rollout status (`6c1c2159e220471e97887187d01d49eb`) | Desktop shows prompt; passive check only, unmodified. |

Before/after captures and the passive reconstructed screens are saved in
`scratch/desktop-recovery-captures.json`; final Desktop captures are included.
The 74 original active/detached Sessions plus these six were inventoried.

## Debug continuation after credential revocation (2026-11-08)

- Re-read the full inventory and the Nov 7 reconnect findings in
  `loopflow.desktop-bug-repors/scratch/incident-codex-reconnect.md`.
- Isolated parser experiment: `uv run --with pyte python` feeding only
  `ESC[<65535u ESC[=5;1u` renders `65535u5;1u`. The literal text in the passive
  pyte captures is therefore not evidence of native draft corruption. No draft
  was erased and no keyboard reset patch was made.
- Production LOO-696 initially captured its retained `whats wsas` draft;
  explicit Desktop focus succeeded. A later capture returned `The retained pane
  has no native surface; no client was acquired.` No input or native restart
  was performed on 696 during this pass. Physical keyboard behavior remains
  unverified; the changing retained surface needs a new observation.
- Exact-Session lookup unnecessarily waited for every historical page, allowing
  an unrelated later page failure to prevent opening a record already found.
  It now stops at that record and merges it into the latest inventory. The
  regression supplies a failing later page and checks the selected conversation
  and retained unrelated record. This is an established code failure, not a
  demonstrated explanation of every production opening failure.
- Production LOO-793 returned `opening` while its selected record was absent;
  the Dev opening later reported `Opening canceled by navigation or a newer
  request.` Do not count either receipt as recovery. MTG-243/244 captures still
  returned `no native surface`. MTG-245 initially captured its quota dialog and
  empty composer in production.
- `lf account next --json` reported Claude blocked (both logins cooling), and
  Codex `jackstah-1066ea9c99d1` ready. A bounded same-Session MTG-245 move with
  `--account codex=jackstah@gmail.com` displayed the old client's exact error:
  `this store needs an lf that knows task_session_promotion (it has task_session_promotion; latest known wave_removal); run lf install`.
  The replacement displayed Codex's sign-in screen. The Session record changed
  from `manabot-eng` to `jackstah-1066ea9c99d1`; native PID 44361 started at
  18:30:07 local. No model turn, credit purchase or goal resume was submitted.
  The temporary viewer detached with Ctrl+]. Desktop switch/open was requested
  again, but the first capture had no native surface. Cached account state says
  connected for this login while provider-level credentials say expired;
  this discrepancy is not yet diagnosed. Do not bulk-move the remaining clients
  based only on `account next`.
- Parent native PID 30832 was freshly confirmed alive with start time
  15:39:36 local. No protected process was signaled. No missing-socket owner was
  stopped; the Nov 7 missing provider endpoints and current owner sockets remain
  distinct failures.

Checks: `swift test --package-path swift --filter
'WorkDestinationTests|SessionsStoreTests|DesktopPaneControlTests'`: 129 passed.
The first pass exposed three fixtures still expecting unqualified task-location
commands from before this branch's repository fix, plus an old test expecting
exact opening to enumerate unrelated history. Fixtures now use the scoped
command; full inventory pagination is still tested separately from opening.
`uv run python scripts/loopflow-dev.py install`: passed (existing Ghostty symbol
warnings); installed Dev only. `git diff --check`: passed. No Rust changes in
this pass; no publication or merge. Production installation remains unchanged.
Raw observations and logs: `/tmp/loo813-recovery/debug-*`.

Remaining: establish why the ready Codex route reaches sign-in, restore and
verify production retained surfaces including 696/434/engine-performance, inspect
native drafts for the missing-owner clients, and implement safe interactive
quota fallback. This incident is not fully repaired and is not ready to be
reported as all Sessions working.

## Continued in the fork, 2026-11-08 19:00–19:15 local

- Protected continuation: `ce8db5ba4fcb45a59769b7aaacf19d8a`. The earlier
  headless debug Session was closed; no Graph was running.
- A fresh `account/read` against the stored `jackstah-1066ea9c99d1` login
  returned JSON-RPC error `-32603` with the phrases `refresh token`, `revoked`,
  and `sign in`. Only those phrases and the error code were emitted by the
  diagnostic. Account inspection called it `unavailable` and retained
  `connected`: subscription polling did not use the agent runner's existing
  revoked-token classification. The regression now checks both definitive
  rejection and recovery from a working copy of the same login.
- `lf account codex=loopflow-eng@loopflow.studio --json` freshly reported
  `verification: accepted`, active, with a weekly-capacity reading. This differs
  from the earlier missing-credential observation; no reconnect was performed
  by this continuation.
- Both Desktop applications were closed at the first current app inventory.
  A Desktop state request timed out against its leftover socket. Opening
  `/Users/jack/Applications/Loopflow Dev.app` restored API responses.
- Exact LOO-696 opening in the earlier Dev build ended with `Opening was
  closed or released.` while startup still showed saved Work. A regression
  reproduced this: navigation detached the saved workspace and cleared the
  opening request assigned immediately before navigation. Assigning that
  request afterward lets the addressed Session install. Native verification
  of the rebuilt app is still pending below.
- Official Codex app-server docs describe `account/read` as refreshing the
  current managed login and `chatgptAuthTokens` as an experimental mode whose
  host owns token refresh. They do not establish that managed accounts can be
  switched in place while retaining a native composer. No external-token mode
  was added: https://learn.chatgpt.com/docs/app-server#auth-endpoints .

Checks: revoked-refresh regression 1 passed; same-login recovery suite 7 passed;
Swift WorkDestination/SessionsStore/DesktopPaneControl suites 130 passed;
`cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed. Rust test
linking reported its existing large `__eh_frame` warning. Dev build/install
and native recovery checks follow; automatic interactive fallback and the
twenty uncertain native clients remain unfinished.

### Subscription truncation, 19:13–19:20 local

The rebuilt Dev app's LOO-696 opening reached `usable`, then its current
Session reading said `Selected Session is absent from the current Session
reading`; capture again reported no native surface. The exact Session still
existed, active, and an unlimited repository list contained it among 348 rows.
The server's unpaged `Read::SessionList` omitted `--limit`, inheriting the CLI's
100-row default. Its next subscription frame discarded older exact openings.
The unpaged served read and Swift CLI fallback now request `--limit 0`; paged
reads remain bounded. A 101-Session regression exercises the parsed filter
against SQLite. The shared server must be updated for native verification.

Live account inspection with the fixed binary reported the jackstah login
`missing`/`rejected` and supplied `lf account connect codex=jackstah@gmail.com`.
It is no longer advertised as connected by cached routing.

Installation preview found the shared store already at `server_request_stops`
and all 21 executable references resolvable, with no migration to apply. A
CLI-only preview refused because the installed app and CLI must update
together. No CLI-only installation was attempted.

### Production checks after promotion, 19:23–19:34 local

Promoted the matching signed app and CLI to `/Applications/Loopflow.app` and
`~/.local/bin/lf`; no schema migration. The service reports revision
`64cffb3cc`. The installer retained the prior artifacts. Added the existing
Take back operation to the Desktop action registry for the same addressed-pane
control as Refresh; CUA still returned `cgWindowNotFound`.

- LOO-696 remained in the current inventory after subscription updates. Its
  current native client was at Codex sign-in, unlike the earlier draft
  observation. Reopened its saved thread with the freshly verified engineering
  login; it reached the composer and reported 21% weekly capacity. The old token
  error is in saved output. No model turn was submitted. Production Take back
  acquired its terminal; captured `x`, deleted it, and captured the empty
  composer again. Refresh and navigation retained the conversation.
- Production LOO-434 and LOO-718 acquired their native terminals. Each captured
  `x`, then an empty composer after Backspace. LOO-434 survived Refresh. LOO-718
  still displayed `Goal hit usage limits (/goal resume)`; no goal resumed.
- A fresh passive inventory found no Codex sign-in screen among reachable
  clients. MTG-245 already displayed a prompt and 21% weekly capacity; this
  continuation did not change it. The original twenty missing-socket clients
  still failed the passive connection check; no replacement was attempted.

Checks: Swift SessionsStore/DesktopPaneControl suites: 62 passed after adding
the Take back action and its DTO fixture; `git diff --check` passed. Earlier
151-test Swift, Rust focused tests and clippy results apply to unchanged code.
Current observations: `/tmp/loo813-current/`; production captures:
`/tmp/loo813-696-after-refresh.txt`. Automatic interactive account/provider
fallback remains unimplemented; this is not an all-Sessions recovery claim.

Engine-performance's production pane also captured `x` and acknowledged
Backspace. Capture after Refresh timed out twice. Samples from production PID
14527 show the main thread in Task-index rebuilding and SwiftUI attribute
comparison, not waiting on a provider. Both the app and service were debug
builds; replacing them with normal release builds is the next check.
Samples: `/tmp/loo813-desktop-stall.sample`,
`/tmp/loo813-desktop-stall2.sample`. No extra keystroke was sent after the
timeout.

### Release installation, 2026-11-09 morning

The build processes survived the overnight sleep. `swift build --package-path
swift -c release` and `cargo build --release -p loopflow --bin lf` passed. Cargo
recompiled locally after sccache stopped. The signed release bundle and CLI were
promoted together at `a1cf57374`; no migration. Prior installation artifacts
remain available through the installer.

Both repository windows then reported current readings. Production captures of
LOO-696, LOO-434, LOO-718 and engine-performance succeeded. Refresh completed for
696, 434 and engine-performance, followed by successful captures. LOO-696 again
captured `x` and then its empty composer after Backspace. LOO-718 still displayed
`Goal hit usage limits (/goal resume)`. No goal resumed or model turn submitted.
The debug-build stall did not recur during these release checks; its root cause
has not been established. Physical keyboard interaction remains unverified.

Release logs: `/tmp/loo813-release-build.log`, `/tmp/loo813-release-cli.log`,
`/tmp/loo813-promote-release.log`. Session readings:
`/tmp/loo813-release-sessions.json`. The automatic interactive quota handoff and
the pending choice about replacing the twenty uncertain clients remain open.
