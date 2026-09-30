# Complete CLI catalog · LOO-338

Jack Heart requested this catalog before implementation on 2026-09-30. Baseline: `a6b1bc3df`. The compiled Clap tree is authoritative; historical docs supplied design evidence only. All rows have a verdict. A rename retains behavior unless its row explicitly states otherwise. The target is not a claim of implementation.

## Counts and reproduction

- Before: **141 commands below the root**, 18 hidden; 142 rows including the root. **440 flag entries** (including automatic help/version), **95 positional arguments**, **10 extra aliases** (one command alias, nine flag aliases).
- Current slice: **140 commands below root**, 18 hidden; **437 flag entries**, **94 positionals**, **0 extra aliases**. Compiled [current extraction](cli-catalog-current.json). This is an intermediate measurement, not completion of the owner tree. Final demo counts must be regenerated after all verdicts.
- `cargo run -p loopflow --example cli_catalog > scratch/cli-catalog-before.json` — passed against the unchanged parser.
- `target/debug/lf help --all` in a disposable Home — exit 0, 125 lines, no Home state created; public help omits hidden commands and options by design.
- Raw [Clap metadata](cli-catalog-before.json) and [public help](cli-help-before.txt) accompany this catalog. Stable C/A identifiers below link research to rows.

## Evidence rules

Caller citations distinguish executable/agent references, documented public use, and tests. A source reference is not proof of live use. Option rows cite their declaration and the owning command's callers; that does not assert those callers pass every optional flag. No repository match does not prove no external user exists. Retain a public option only when it changes a distinct real behavior; delete proven no-ops and duplicate aliases. Dynamic external definitions are not enumerable commands: root/skill/flow fallthrough and the bare default are accounted for below.

Keep means retain the real operation at its owner; it does not waive the required overviews, first-result path, truthful JSON/errors or child readiness. R01–R10 in [fresh research](cli-research-20260930.md) add these cross-cutting acceptance requirements. Real product choices remain explicitly at demo; current behavior is retained until Jack decides.





## Commands

P:LINE refers to the [baseline Clap declarations](https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/mod.rs). E references are caller/source evidence. N references expand in the rationale section. Option caller references inherit the named command row; they do not claim every optional input is passed by that caller.

| Row | Canonical path | Target owner | Purpose | Callers / source | Overlap | Verdict → target |
|---|---|---|---|---|---|---|
| C000 | lf | root | Open Loopflow or run its CLI | dispatch: [E000]; P:14 | N000 | keep → lf |
| C001 | lf user | lf home | Inspect the current user | agent instructions: [E001]; documented public use: [E002]; documented public use: [E003]; P:225 | N001 | rename → lf home user |
| C002 | lf user name | lf home user | Show the display name from personal Loopflow configuration or Git | Desktop: [E004]; runtime/source reference: [E005]; agent instructions: [E001]; P:216 | N001 | rename → lf home user name |
| C003 | lf : | lf | Run an inline prompt | documented public use: [E002]; documented public use: [E006]; documented public use: [E007]; P:231 | N001 | keep → lf : |
| C004 | lf desktop | lf home | Open or focus Loopflow.app | agent instructions: [E008]; documented public use: [E002]; P:236 | N001 | rename → lf home desktop |
| C005 | lf screenshot | lf home | Capture a URL or local HTML file without claiming the user's browser | agent instructions: [E009]; agent instructions: [E010]; documented public use: [E011]; P:238 | N001 | rename → lf home screenshot |
| C006 | lf __screenshot-supervisor (hidden) | lf | Internal owner-loss supervisor for one browser capture | runtime/source reference: [E012]; documented public use: [E011]; documented public use: [E013]; P:244 | N001 | keep → lf __screenshot-supervisor |
| C007 | lf __provider-session (hidden) | lf | Internal provider callback that records one native interactive session | runtime callback: [E014]; P:250 | N001 | keep → lf __provider-session |
| C008 | lf ask | lf session | Open a durable session and wait for the user to complete it | agent instructions: [E015]; agent instructions: [E016]; agent instructions: [E017]; P:252 | N002 | rename → lf session ask |
| C009 | lf session | lf | Inspect and continue Sessions | Desktop: [E018]; runtime/source reference: [E019]; runtime/source reference: [E020]; P:257 | N001 | keep → lf session |
| C010 | lf session list | lf session | List Sessions | Desktop: [E021]; Desktop: [E022]; agent instructions: [E023]; P:728 | N001 | keep → lf session list |
| C011 | lf session open | lf session | Open or resume one session in this terminal | Desktop: [E024]; Desktop: [E025]; runtime/source reference: [E020]; P:736 | N003 | keep → lf session open |
| C012 | lf session complete | lf session | Complete a review, blocked Ask, or interactive session | Desktop: [E026]; Desktop: [E027]; runtime/source reference: [E028]; P:748 | N001 | keep → lf session complete |
| C013 | lf session rename | lf session | Rename a Session; a human name is never replaced by a suggestion | Desktop: [E029]; Desktop: [E018]; agent instructions: [E030]; P:750 | N001 | keep → lf session rename |
| C014 | lf session ready | lf session | Mark the active session ready for your review | runtime/source reference: [E019]; runtime/source reference: [E031]; runtime/source reference: [E032]; P:761 | N004 | keep → lf session ready |
| C015 | lf session serve-flow (hidden) | lf session | Run the exact review skill in its durable terminal | runtime child: [E033]; P:767 | N001 | keep → lf session serve-flow |
| C016 | lf session serve-ask (hidden) | lf session | Run one ad-hoc request in its durable terminal | runtime child: [E034]; P:777 | N001 | keep → lf session serve-ask |
| C017 | lf session stop-run (hidden) | lf session | Stop one exact native provider Run after its review completes | runtime child: [E035]; P:780 | N005 | rename → lf session stop-exec |
| C018 | lf install | lf home | Install the latest published Loopflow release from any directory | script: [E036]; runtime/source reference: [E037]; runtime/source reference: [E038]; P:262 | N006 | rename → lf home install |
| C019 | lf install schedule | lf home install | Install the latest Loopflow at login and weekly by default (macOS launchd) | runtime/source reference: [E039]; documented public use: [E040]; test: [E041]; P:1148 | N001 | rename → lf home install schedule |
| C020 | lf install recover-switch (hidden) | lf home install | Continue one interrupted machine install switch from its pinned candidate | runtime child: [E042]; P:1155 | N001 | rename → lf home install recover-switch |
| C021 | lf install preflight (hidden) | lf home install | Preview whether this build may replace the global lf (read-only). Reads the shared store's migration frontier and validates executable planning references against this binary; mutates nothing and exits non-zero on refusal so a caller can gate on it | script: [E043]; runtime/source reference: [E044]; runtime/source reference: [E045]; P:1165 | N001 | rename → lf home install preflight |
| C022 | lf install local-preflight (hidden) | lf home install | Validate this exact local candidate against one receipt-selected store | runtime/source reference: [E046]; test: [E047]; test: [E048]; P:1172 | N001 | rename → lf home install local-preflight |
| C023 | lf install advance-switch (hidden) | lf home install | Advance the receipt-selected store with this exact candidate's registry | runtime child: [E049]; P:1180 | N001 | rename → lf home install advance-switch |
| C024 | lf install promote (hidden) | lf home install | Promote this build to the global CLI: content-address it into ~/.lf/bin and atomically repoint the target symlink, under the exclusive promotion lock. Refuses — leaving every target unchanged — on incompatible schema or persisted executable evidence | script: [E050]; runtime/source reference: [E051]; runtime/source reference: [E052]; P:1189 | N001 | rename → lf home install promote |
| C025 | lf install rollback (hidden) | lf home install | Repoint the global CLI at retained prior bytes only after that binary's own preflight proves it recognizes the current store frontier | No literal caller found; usage unestablished; P:1230 | N001 | rename → lf home install rollback |
| C026 | lf pr | lf task | Pull request lifecycle | Desktop: [E053]; runtime/source reference: [E054]; runtime/source reference: [E055]; P:267 | N001 | rename → lf task pr |
| C027 | lf pr checks | lf task pr | Show CI status for current branch | test: [E056]; test: [E057]; test: [E058]; P:1249 | N007 | rename → lf task pr checks |
| C028 | lf pr status | lf task pr | Show current branch's PR state | No literal caller found; usage unestablished; P:1257 | N001 | rename → lf task pr status |
| C029 | lf pr next | lf task pr | After an out-of-band merge, rotate this Task to its next serial PR, carrying committed and uncommitted follow-up onto the new branch | runtime/source reference: [E054]; test: [E059]; P:1260 | N008 | rename → lf task pr next |
| C030 | lf pr publish | lf task pr | Publish a ready PR headlessly: push, create or refresh, print state + URL. Opens no review surface | Desktop: [E060]; agent instructions: [E061]; agent instructions: [E062]; P:1267 | N009 | rename → lf task pr publish |
| C031 | lf pr open | lf task pr | Push and create or update a draft PR, then open its GitHub page. Existing ready PRs stay ready; opening a draft does not publish it | Desktop: [E053]; runtime/source reference: [E055]; runtime/source reference: [E063]; P:1277 | N010 | rename → lf task pr open |
| C032 | lf pr submit | lf task pr | Prepare a PR to land: rebase, clear scratch, mark ready, and assign it to you. Nothing merges until you click merge on GitHub | agent instructions: [E064]; agent instructions: [E065]; agent instructions: [E066]; P:1287 | N011 | rename → lf task pr submit |
| C033 | lf pr arm | lf task pr | Prepare a PR, request exact-head auto-merge, and return without watching | runtime/source reference: [E067]; agent instructions: [E068]; agent instructions: [E069]; P:1306 | N012 | rename → lf task pr arm |
| C034 | lf pr land | lf task pr | Arm and watch a PR through CI repair and authoritative merge | runtime/source reference: [E070]; runtime/source reference: [E071]; runtime/source reference: [E072]; P:1325 | N013 | rename → lf task pr land |
| C035 | lf pr abandon | lf task pr | Abandon branch: close PR, remove worktree, delete branch | runtime/source reference: [E054]; agent instructions: [E073]; test: [E074]; P:1344 | N014 | rename → lf task pr abandon |
| C036 | lf wt | lf task | Worktree operations | runtime/source reference: [E075]; runtime/source reference: [E076]; runtime/source reference: [E077]; P:272 | N001 | rename → lf task worktree |
| C037 | lf wt create | lf task worktree | Create a low-level sibling worktree | runtime/source reference: [E077]; test: [E078]; P:1638 | N015 | rename → lf task worktree create |
| C038 | lf wt switch | lf task worktree | Switch to a worktree by name, identity leaf, or full branch | runtime/source reference: [E075]; test: [E079]; P:1646 | N001 | rename → lf task worktree switch |
| C039 | lf wt list | lf task worktree | List worktrees (read-only; reflects the last-synced main) | runtime/source reference: [E076]; agent instructions: [E080]; documented public use: [E081]; P:1651 | N016 | rename → lf task worktree list |
| C040 | lf wt prune | lf task worktree | Remove clean terminal or inactive worktrees | documented public use: [E082]; test: [E083]; P:1662 | N001 | rename → lf task worktree prune |
| C041 | lf wt remove | lf task worktree | Remove a worktree | documented public use: [E084]; test: [E085]; P:1669 | N001 | rename → lf task worktree remove |
| C042 | lf rebase | lf task | Rebase current branch onto target (default: main) | runtime/source reference: [E086]; runtime/source reference: [E087]; runtime/source reference: [E088]; P:277 | N001 | rename → lf task rebase |
| C043 | lf commit | lf task | Commit changes | agent instructions: [E089]; agent instructions: [E090]; agent instructions: [E091]; P:297 | N001 | rename → lf task commit |
| C044 | lf auth | lf | Provider authentication for local lf skills and ops | script: [E092]; runtime/source reference: [E093]; runtime/source reference: [E094]; P:306 | N001 | rename → lf account |
| C045 | lf auth status | lf account | Inspect cached credentials and subscription windows; verify explicitly | script: [E092]; script: [E095]; script: [E096]; P:1493 | N017 | rename → lf account status |
| C046 | lf auth disconnect | lf account | Disconnect local credentials or one managed login | documented public use: [E097]; P:1503 | N001 | rename → lf account disconnect |
| C047 | lf auth connect | lf account | Connect local credentials or a managed login using a remembered browser | runtime/source reference: [E093]; runtime/source reference: [E094]; runtime/source reference: [E098]; P:1508 | N018 | rename → lf account connect |
| C048 | lf auth set | lf account | Edit account configuration or remembered browser choices | script: [E099]; documented public use: [E100]; test: [E101]; P:1521 | N019 | rename → lf account set |
| C049 | lf auth route | lf account | Configure and inspect managed account routing | script: [E102]; runtime/source reference: [E103]; agent instructions: [E104]; P:1545 | N001 | rename → lf account route |
| C050 | lf auth route set | lf account route | Replace a provider's ordered route | script: [E102]; runtime/source reference: [E103]; documented public use: [E105]; P:1554 | N001 | rename → lf account route set |
| C051 | lf auth route show | lf account route | Explain configured and automatic account selection | script: [E106]; agent instructions: [E104]; documented public use: [E107]; P:1564 | N001 | rename → lf account route show |
| C052 | lf release | lf repo | Release operations (run, check, notes, bump, tag, status) | script: [E108]; script: [E109]; script: [E110]; P:311 | N001 | rename → lf repo release |
| C053 | lf release run | lf repo release | Run the full release workflow end-to-end | script: [E108]; script: [E111]; runtime/source reference: [E112]; P:1577 | N020 | rename → lf repo release run |
| C054 | lf release check | lf repo release | Check if PRs have merged since the last tag | runtime/source reference: [E113]; documented public use: [E114]; P:1584 | N001 | rename → lf repo release check |
| C055 | lf release notes | lf repo release | Generate release notes for a version | No literal caller found; usage unestablished; P:1589 | N001 | rename → lf repo release notes |
| C056 | lf release bump | lf repo release | Bump version in manifest files | No literal caller found; usage unestablished; P:1601 | N001 | rename → lf repo release bump |
| C057 | lf release tag | lf repo release | Create a git tag and push it | No literal caller found; usage unestablished; P:1608 | N001 | rename → lf repo release tag |
| C058 | lf release publish | lf repo release | Stage or publish a GitHub Release | script: [E115]; test: [E116]; P:1615 | N021 | rename → lf repo release publish |
| C059 | lf release status | lf repo release | Check release workflow status | No literal caller found; usage unestablished; P:1629 | N001 | rename → lf repo release status |
| C060 | lf repo | lf | Repository provider administration | runtime/source reference: [E117]; runtime/source reference: [E118]; documented public use: [E119]; P:316 | N001 | keep → lf repo |
| C061 | lf repo reteam | lf repo | Reconcile linked Waves to the repository's Linear Team | runtime/source reference: [E117]; runtime/source reference: [E118]; documented public use: [E120]; P:1444 | N001 | keep → lf repo reteam |
| C062 | lf repo webhook | lf repo | Register or serve Linear webhooks | documented public use: [E121]; test: [E122]; P:1449 | N001 | keep → lf repo webhook |
| C063 | lf repo webhook serve | lf repo webhook | Run the receiver that turns Linear edits into Task direction. Reads the signing secret from LF_LINEAR_WEBHOOK_SECRET (source it from Doppler) | documented public use: [E123]; P:1459 | N001 | keep → lf repo webhook serve |
| C064 | lf repo webhook register | lf repo webhook | Register the Issue/Comment webhook with Linear (one-time). Reads the signing secret from LF_LINEAR_WEBHOOK_SECRET | documented public use: [E121]; P:1466 | N001 | keep → lf repo webhook register |
| C065 | lf home | lf | Inspect this Home and observe routes to other Homes | script: [E124]; runtime/source reference: [E125]; agent instructions: [E126]; P:321 | N001 | keep → lf home |
| C066 | lf home id | lf home | Print this machine's stable local Home identity | script: [E127]; runtime/source reference: [E128]; agent instructions: [E126]; P:1477 | N022 | keep → lf home id |
| C067 | lf home observe | lf home | Record the current route for a known Home identity | script: [E129]; runtime/source reference: [E130]; agent instructions: [E131]; P:1482 | N023 | keep → lf home observe |
| C068 | lf sync-skills (hidden) | lf home | Compile loopflow skills into your home vendor Skills directories | documented public use: [E132]; P:327 | N001 | rename → lf home sync-skills |
| C069 | lf cron | lf wave | Local launchd jobs that run lf commands on a schedule | script: [E133]; runtime/source reference: [E134]; runtime/source reference: [E135]; P:336 | N001 | rename → lf wave cron |
| C070 | lf cron add | lf wave cron | Install or replace a scheduled lf invocation | test: [E136]; P:1355 | N001 | rename → lf wave cron add |
| C071 | lf cron list | lf wave cron | List installed loopflow cron jobs | script: [E137]; test: [E138]; test: [E139]; P:1367 | N001 | rename → lf wave cron list |
| C072 | lf cron preflight | lf wave cron | Validate Home authority and declared jobs without changing launchd | script: [E133]; test: [E140]; test: [E141]; P:1376 | N001 | rename → lf wave cron preflight |
| C073 | lf cron sync | lf wave cron | Reconcile installed launchd jobs to match a wave's declared `crons:` | script: [E142]; runtime/source reference: [E135]; test: [E143]; P:1382 | N024 | rename → lf wave cron sync |
| C074 | lf cron run (hidden) | lf wave cron | Execute one installed cron job and persist its terminal receipt | test: [E144]; P:1389 | N001 | rename → lf wave cron run |
| C075 | lf cron history | lf wave cron | Show durable cron receipts | script: [E145]; runtime/source reference: [E134]; runtime/source reference: [E146]; P:1401 | N001 | rename → lf wave cron history |
| C076 | lf cron trigger | lf wave cron | Ask launchd to fire an installed job | script: [E147]; test: [E148]; test: [E149]; P:1416 | N001 | rename → lf wave cron trigger |
| C077 | lf cron remove | lf wave cron | Uninstall a scheduled lf invocation | test: [E150]; P:1431 | N001 | rename → lf wave cron remove |
| C078 | lf wave | lf | Serve a Wave or rotate its chapter plan | Desktop: [E151]; Desktop: [E152]; Desktop: [E153]; P:341 | N001 | keep → lf wave |
| C079 | lf wave list | lf wave | List every wave in the registry (running and stopped), marking which have a live server. Local-only query over the shared ledger | Desktop: [E151]; Desktop: [E152]; Desktop: [E154]; P:824 | N025 | keep → lf wave list |
| C080 | lf wave status | lf wave | Show one Wave's chapter, Tasks, Runs, and live loop state from the registry. Defaults to the ambient wave (`LF_WAVE_ID`) | Desktop: [E153]; Desktop: [E154]; Desktop: [E155]; P:838 | N001 | keep → lf wave status |
| C081 | lf wave probe | lf wave | Probe a Wave's Home for liveness and the one contextual action | Desktop: [E156]; Desktop: [E157]; runtime/source reference: [E158]; P:859 | N001 | keep → lf wave probe |
| C082 | lf wave connect | lf wave | Connect a Wave to its Initiative and the repository's Team (Task prefix) | runtime/source reference: [E159]; agent instructions: [E160]; agent instructions: [E161]; P:867 | N001 | keep → lf wave connect |
| C083 | lf wave sync | lf wave | Refresh shared planning from Linear | Desktop: [E162]; runtime/source reference: [E163]; runtime/source reference: [E164]; P:884 | N001 | keep → lf wave sync |
| C084 | lf wave rename | lf wave | Rename the provider Initiative | test: [E165]; P:892 | N001 | keep → lf wave rename |
| C085 | lf wave forget | lf wave | Forget an empty Wave registration, preserving authored files | documented public use: [E166]; test: [E167]; P:898 | N001 | keep → lf wave forget |
| C086 | lf wave place | lf wave | Place a Wave on a Home | agent instructions: [E168]; agent instructions: [E169]; documented public use: [E170]; P:906 | N001 | keep → lf wave place |
| C087 | lf wave relocate | lf wave | Rename or rehome a stopped Wave | runtime/source reference: [E171]; documented public use: [E172]; documented public use: [E173]; P:913 | N001 | keep → lf wave relocate |
| C088 | lf wave retire | lf wave | Retire the Wave, retaining history | documented public use: [E174]; P:923 | N001 | keep → lf wave retire |
| C089 | lf wave recover | lf wave | Inspect historical Wave Flow work, or explicitly cancel its saved continuation | runtime/source reference: [E175]; runtime/source reference: [E176]; documented public use: [E177]; P:932 | N001 | keep → lf wave recover |
| C090 | lf wave new-chapter | lf wave | Replace the plan, carry started Tasks, and retire unopened backlog | runtime/source reference: [E178]; runtime/source reference: [E179]; runtime/source reference: [E180]; P:941 | N001 | keep → lf wave new-chapter |
| C091 | lf wave history | lf wave | List chapter boundary receipts | Desktop: [E181]; documented public use: [E182]; documented public use: [E183]; P:955 | N001 | keep → lf wave history |
| C092 | lf wave update-plan | lf wave | Replace the current chapter's KRs, targets, and Flow recommendation | agent instructions: [E184]; agent instructions: [E185]; agent instructions: [E186]; P:962 | N001 | keep → lf wave update-plan |
| C093 | lf __chat-connect (hidden) | lf | Internal: connect chat through the owning Home | Desktop: [E187]; documented public use: [E172]; test: [E188]; P:347 | N001 | keep → lf __chat-connect |
| C094 | lf __resident (hidden) | lf | Internal: the resident body a listener spawns for its own wave. Never booted by hand — the Home daemon owns the listener half | runtime/source reference: [E189]; runtime/source reference: [E190]; runtime/source reference: [E191]; P:359 | N001 | keep → lf __resident |
| C095 | lf task | lf | Linear-backed Task work and bounded workers | Desktop: [E192]; Desktop: [E193]; Desktop: [E194]; P:365 | N001 | keep → lf task |
| C096 | lf task __worker (hidden) | lf task | Internal: drive a Task Flow from its claimed boundary | documented public use: [E195]; documented public use: [E196]; P:974 | N001 | keep → lf task __worker |
| C097 | lf task checkout | lf task | Ensure tracked Task Work and its worktree without starting a worker | Desktop: [E197]; agent instructions: [E198]; agent instructions: [E199]; P:976 | N026 | keep → lf task checkout |
| C098 | lf task run | lf task | Start or continue a Task through its saved Flow | Desktop: [E200]; Desktop: [E193]; runtime/source reference: [E201]; P:989 | N027 | keep → lf task run |
| C099 | lf task create | lf task | File a Task in the current chapter; optionally prepare and run it | Desktop: [E202]; runtime/source reference: [E203]; runtime/source reference: [E204]; P:1008 | N001 | keep → lf task create |
| C100 | lf task status | lf task | Show durable Task facts and current worker evidence | runtime/source reference: [E205]; runtime/source reference: [E206]; runtime/source reference: [E207]; P:1033 | N001 | keep → lf task status |
| C101 | lf task changes | lf task | List files changed from this Task's recorded base commit | Desktop: [E208]; documented public use: [E209]; test: [E210]; P:1040 | N028 | keep → lf task changes |
| C102 | lf task diff | lf task | Show this Task's patch, optionally limited to one changed file | Desktop: [E211]; documented public use: [E212]; test: [E213]; P:1048 | N001 | keep → lf task diff |
| C103 | lf task file | lf task | Read one file from this Task's worktree | Desktop: [E214]; Desktop: [E194]; documented public use: [E215]; P:1060 | N001 | keep → lf task file |
| C104 | lf task save | lf task | Save UTF-8 stdin with an expected revision and retained recovery files | Desktop: [E216]; documented public use: [E217]; P:1070 | N001 | keep → lf task save |
| C105 | lf task complete | lf task | Complete planning work, or a placed Task whose pull requests are settled | documented public use: [E218]; documented public use: [E219]; test: [E220]; P:1079 | N029 | keep → lf task complete |
| C106 | lf task delete | lf task | Delete a Task from Linear and reconcile its local record | runtime/source reference: [E221]; agent instructions: [E222]; documented public use: [E223]; P:1087 | N030 | keep → lf task delete |
| C107 | lf task edit | lf task | Edit a Task's title or notes, before or after placement | Desktop: [E224]; documented public use: [E225]; test: [E226]; P:1089 | N031 | keep → lf task edit |
| C108 | lf task comment | lf task | Read the comment thread, or append direction without starting execution | Desktop: [E192]; Desktop: [E227]; Desktop: [E228]; P:1099 | N032 | keep → lf task comment |
| C109 | lf task interrupt | lf task | Interrupt the active provider turn | Desktop: [E229]; documented public use: [E230]; documented public use: [E231]; P:1108 | N001 | keep → lf task interrupt |
| C110 | lf task wait | lf task | Wait without polling an LM | documented public use: [E232]; documented public use: [E233]; documented public use: [E234]; P:1114 | N001 | keep → lf task wait |
| C111 | lf task restart | lf task | Stop the pinned Flow and begin a new one in a fresh Task worker; defaults to the chapter's currently recommended Flow | Desktop: [E235]; runtime/source reference: [E236]; agent instructions: [E237]; P:1125 | N001 | keep → lf task restart |
| C112 | lf tokens | lf repo | Measure this codebase: lines and tokens per directory (tracked files only) | implementation boundary: [E238]; Desktop: [E239]; Desktop: [E240]; Desktop: [E241]; P:370 | N033 | rename → lf repo tokens |
| C113 | lf usage | lf monitor | Show direct provider-authored usage from Home-local Run records | runtime/source reference: [E242]; runtime/source reference: [E243]; documented public use: [E244]; P:379 | N034 | rename → lf monitor usage |
| C114 | lf __telemetry-scorecard (hidden) | lf | Internal: render the repository maintainer scorecard for telemetry-daily | runtime/source reference: [E245]; test: [E246]; test: [E247]; P:398 | N001 | keep → lf __telemetry-scorecard |
| C115 | lf ci | lf repo | Show how failed CI is detected, repaired, and landed across this Home | runtime/source reference: [E248]; runtime/source reference: [E207]; documented public use: [E249]; P:404 | N001 | rename → lf repo ci |
| C116 | lf ps | lf monitor | Print one parseable snapshot of live Loopflow call trees | script: [E250]; runtime/source reference: [E251]; agent instructions: [E252]; P:419 | N035 | rename → lf monitor ps |
| C117 | lf top | lf monitor | Refresh live Loopflow call trees on a terminal; print once when redirected | runtime/source reference: [E251]; agent instructions: [E253]; documented public use: [E254]; P:425 | N036 | rename → lf monitor top |
| C118 | lf prune | lf monitor | Reap registered orphan providers and remove dead process receipts | runtime/source reference: [E255]; agent instructions: [E256]; documented public use: [E254]; P:431 | N037 | rename → lf monitor prune |
| C119 | lf doctor | lf home | Audit the local run ledger: continuity, vocabulary, attribution, identity, lineage, coverage | Desktop: [E257]; Desktop: [E258]; runtime/source reference: [E259]; P:440 | N038 | rename → lf home doctor |
| C120 | lf list | lf | Discover commands, skills, and flows | CI: [E260]; runtime/source reference: [E261]; runtime/source reference: [E262]; P:449 | N039 | keep → lf list |
| C121 | lf help | lf | Explain a command, skill, or flow without launching it | runtime/source reference: [E262]; documented public use: [E132]; documented public use: [E263]; P:455 | N040 | keep → lf help |
| C122 | lf roadmap | lf task | Show the current repository's roadmap: every open Task across the repo's Waves, joined to live evidence and bucketed into Now / Waiting / Available / Later. `--wave` scopes it; `--all` spans every repository on this machine. Local-only, deterministic | Desktop: [E264]; Desktop: [E153]; Desktop: [E265]; P:464 | N041 | rename → lf task list |
| C123 | lf activity | lf monitor | Show one ordered record of durable Work, Run, PR, and Steer facts | Desktop: [E266]; runtime/source reference: [E267]; runtime/source reference: [E268]; P:476 | N042 | rename → lf monitor activity |
| C124 | lf runs | lf monitor | Show recent agent-backed skill runs with context and token evidence | Desktop: [E269]; Desktop: [E240]; Desktop: [E270]; P:497 | N043 | rename → lf monitor list |
| C125 | lf replay | lf monitor | Launch the exact provider request recorded by a prior Run as a child Run | documented public use: [E271]; documented public use: [E272]; documented public use: [E273]; P:545 | N044 | rename → lf monitor replay |
| C126 | lf reply | lf wave | Observe wave-chat message(s) and print a reply only if one is warranted. A direct capability: no listener, resident, or governance loop | runtime/source reference: [E274]; runtime/source reference: [E275]; documented public use: [E172]; P:551 | N001 | rename → lf wave reply |
| C127 | lf chat | lf wave | Converse with a Wave; --follow replays its thread | Desktop: [E276]; Desktop: [E277]; runtime/source reference: [E278]; P:565 | N001 | rename → lf wave chat |
| C128 | lf op (hidden) | root | Removed; the operations are top-level (`lf pr`, `lf rebase`, `lf wt`, `lf task`) | documented public use: [E279]; test: [E280]; P:598 | N045 | delete → — |
| C129 | lf ssh | lf home | Run lf on a Home or SSH host carrying your local credentials | runtime/source reference: [E281]; runtime/source reference: [E282]; runtime/source reference: [E283]; P:610 | N001 | rename → lf home ssh |
| C130 | lf run | lf | Run a definition, preferring a flow over a same-named skill | runtime/source reference: [E262]; documented public use: [E195]; documented public use: [E284]; P:646 | N046 | keep → lf run |
| C131 | lf flow | lf | Run or inspect authored flows | runtime/source reference: [E285]; runtime/source reference: [E286]; runtime/source reference: [E287]; P:652 | N047 | keep → lf flow |
| C132 | lf flow list | lf flow | List authored flows | Desktop: [E288]; runtime/source reference: [E287]; documented public use: [E289]; P:683 | N001 | keep → lf flow list |
| C133 | lf flow show | lf flow | Inspect the expanded steps of a flow | documented public use: [E290]; test: [E291]; P:688 | N048 | keep → lf flow show |
| C134 | lf flow validate | lf flow | Validate a flow and its review points | documented public use: [E292]; test: [E293]; P:690 | N001 | keep → lf flow validate |
| C135 | lf flow decide | lf flow | Record a decision for the current Flow boundary | runtime/source reference: [E285]; runtime/source reference: [E294]; documented public use: [E295]; P:692 | N049 | keep → lf flow decide |
| C136 | lf flow route | lf flow | Select an authored branch | runtime/source reference: [E286]; documented public use: [E296]; documented public use: [E297]; P:699 | N001 | keep → lf flow route |
| C137 | lf flow blocked | lf flow | Open a Session to resolve a blocked decision | runtime/source reference: [E285]; runtime/source reference: [E298]; documented public use: [E299]; P:701 | N001 | keep → lf flow blocked |
| C138 | lf flow resume | lf flow | Continue a saved Flow invocation | runtime/source reference: [E300]; runtime/source reference: [E301]; documented public use: [E302]; P:706 | N050 | keep → lf flow resume |
| C139 | lf skill | lf | Run or inspect skills | runtime/source reference: [E303]; runtime/source reference: [E304]; runtime/source reference: [E262]; P:657 | N051 | keep → lf skill |
| C140 | lf skill list | lf skill | List skills, optionally inside a namespace | runtime/source reference: [E304]; documented public use: [E305]; P:669 | N001 | keep → lf skill list |
| C141 | lf skill show | lf help | Inspect a skill without launching it | documented public use: [E306]; test: [E307]; P:675 | N052 | merge into → lf help skill |

## Options and positional arguments

Primary short and long flags share one row. Hidden and automatic arguments are included; required/default metadata remains in the raw JSON. Purpose and distinct effect justify retained options; literal caller absence alone is not evidence of a dead public input.

| Row | Canonical path and argument | Owner / callers | Purpose | Overlap / source | Verdict → target |
|---|---|---|---|---|---|
| A000 | lf --docs | C000 | Docs paths, globs, or directories to include in context | N053; P:20 | keep → --docs |
| A001 | lf --clipboard / -c | C000 | Include clipboard content in prompt | N053; P:24 | keep → --clipboard |
| A002 | lf --model / -m | C000 | Model to use (harness or harness:model) | N053; P:28 | keep → --model |
| A003 | lf --account | C000 | Prefer this managed provider login before the normal route. Repeat to select provider-qualified preferences such as `claude=jack@`. Logins spend; a profile is only the Chrome venue accounts log in through, so it is never a run-time selector | N054; P:40 | keep → --account |
| A004 | lf --only-account | C000 | Restrict this invocation and its children to exactly these managed provider logins. Providers without a selection are unavailable | N054; P:50 | keep → --only-account |
| A005 | lf --__account-lease-probe (hidden) | C000 | Internal SSH compatibility and broker-connectivity probe | N053; P:54 | keep → --__account-lease-probe |
| A006 | lf --yolo | C000 | Skip permission prompts | N055; P:58 | keep → --yolo |
| A007 | lf --interactive / -i | C000 | Run interactively | N056; P:62 | keep → --interactive |
| A008 | lf --batch / -b | C000 | Run in batch/headless mode | N056; P:66 | keep → --batch |
| A009 | lf --tui | C000 | Hand off Claude, Codex, or OpenCode to the terminal (overrides session.launch) | N053; P:70 | keep → --tui |
| A010 | lf --ide | C000 | Hand off Claude or Codex to the vendor app (overrides session.launch) | N053; P:74 | keep → --ide |
| A011 | lf --chrome | C000 | Enable Chrome integration (Claude) | N057; P:78 | keep → --chrome |
| A012 | lf --no-chrome | C000 | Disable Chrome integration (Claude) | N057; P:82 | keep → --no-chrome |
| A013 | lf --diff-files | C000 | Include files changed on branch | N057; P:86 | keep → --diff-files |
| A014 | lf --no-diff-files | C000 | Exclude files changed on branch | N057; P:90 | keep → --no-diff-files |
| A015 | lf --diff | C000 | Include raw git diff | N057; P:94 | keep → --diff |
| A016 | lf --no-diff | C000 | Exclude raw git diff | N057; P:98 | keep → --no-diff |
| A017 | lf --max-turns | C000 | Maximum agent turns for this invocation | N053; P:102 | keep → --max-turns |
| A018 | lf --wave / -w | C000 | Select Wave Work, or qualify a selected Task | N058; P:106 | keep → --wave |
| A019 | lf --task | C000 | Select Task Work | N058; P:110 | keep → --task |
| A020 | lf --as | C000 | Select one Work for a direct skill, flow or inline prompt | N053; P:118 | keep → --as |
| A021 | lf --__cwd (hidden) | C000 | Keep a Work-bound internal launch in this exact checkout | N053; P:122 | keep → --__cwd |
| A022 | lf --no-loopflow | C000 | Exclude loopflow operating guidance | N053; P:126 | keep → --no-loopflow |
| A023 | lf --help / -h | C000 | Explain this canonical command without performing it | N059; P:14 | keep → --help |
| A024 | lf --version / -V | C000 | Print version | N053; P:1579 | keep → --version |
| A025 | lf user --help / -h | C001 | Explain this canonical command without performing it | N059; P:225 | keep → --help |
| A026 | lf user name --json | C002 | Emit structured evidence for this operation | N060; P:218 | keep → --json |
| A027 | lf user name --help / -h | C002 | Explain this canonical command without performing it | N059; P:216 | keep → --help |
| A028 | lf : <prompt> | C003 | Select prompt | N053; P:233 | keep → <prompt> |
| A029 | lf : --help / -h | C003 | Explain this canonical command without performing it | N059; P:231 | keep → --help |
| A030 | lf desktop --help / -h | C004 | Explain this canonical command without performing it | N059; P:236 | keep → --help |
| A031 | lf screenshot <source> | C005 | URL or local HTML file to capture | N053; P:198 | keep → <source> |
| A032 | lf screenshot --output / -o | C005 | PNG destination | N053; P:202 | keep → --output |
| A033 | lf screenshot --width | C005 | Viewport width in pixels | N053; P:206 | keep → --width |
| A034 | lf screenshot --height | C005 | Viewport height in pixels | N053; P:210 | keep → --height |
| A035 | lf screenshot --help / -h | C005 | Explain this canonical command without performing it | N059; P:238 | keep → --help |
| A036 | lf __screenshot-supervisor <source> | C006 | URL or local HTML file to capture | N053; P:198 | keep → <source> |
| A037 | lf __screenshot-supervisor --output / -o | C006 | PNG destination | N053; P:202 | keep → --output |
| A038 | lf __screenshot-supervisor --width | C006 | Viewport width in pixels | N053; P:206 | keep → --width |
| A039 | lf __screenshot-supervisor --height | C006 | Viewport height in pixels | N053; P:210 | keep → --height |
| A040 | lf __screenshot-supervisor --help / -h | C006 | Explain this canonical command without performing it | N059; P:244 | keep → --help |
| A041 | lf __provider-session --help / -h | C007 | Explain this canonical command without performing it | N059; P:250 | keep → --help |
| A042 | lf ask --skill | C008 | Named skill for the session | N053; P:719 | keep → --skill |
| A043 | lf ask <question> | C008 | What the session should work through | N053; P:722 | keep → <question> |
| A044 | lf ask --help / -h | C008 | Explain this canonical command without performing it | N059; P:252 | keep → --help |
| A045 | lf session --help / -h | C009 | Explain this canonical command without performing it | N059; P:257 | keep → --help |
| A046 | lf session list --json | C010 | Emit structured evidence for this operation | N060; P:730 | keep → --json |
| A047 | lf session list --all | C010 | Include waiting steps from every repository on this machine | N053; P:733 | keep → --all |
| A048 | lf session list --help / -h | C010 | Explain this canonical command without performing it | N059; P:728 | keep → --help |
| A049 | lf session open <id> | C011 | Select id | N053; P:737 | keep → <id> |
| A050 | lf session open --json | C011 | Emit structured evidence for this operation | N060; P:739 | keep → --json |
| A051 | lf session open --replace | C011 | Stop Loopflow-owned clients before resuming here | N053; P:742 | keep → --replace |
| A052 | lf session open --try | C011 | Ask the provider to resume even when another client is active | N053; P:745 | keep → --try |
| A053 | lf session open --help / -h | C011 | Explain this canonical command without performing it | N059; P:736 | keep → --help |
| A054 | lf session complete <id> | C012 | Select id | N053; P:748 | keep → <id> |
| A055 | lf session complete --help / -h | C012 | Explain this canonical command without performing it | N059; P:748 | keep → --help |
| A056 | lf session rename <id> | C013 | Select id | N053; P:751 | keep → <id> |
| A057 | lf session rename <name> | C013 | Select name | N053; P:753 | keep → <name> |
| A058 | lf session rename --suggest | C013 | Propose an agent-generated name; keeps a human-assigned name | N053; P:756 | keep → --suggest |
| A059 | lf session rename --json | C013 | Emit structured evidence for this operation | N060; P:758 | keep → --json |
| A060 | lf session rename --help / -h | C013 | Explain this canonical command without performing it | N059; P:750 | keep → --help |
| A061 | lf session ready <summary> | C014 | Select summary | N053; P:763 | keep → <summary> |
| A062 | lf session ready --help / -h | C014 | Explain this canonical command without performing it | N059; P:761 | keep → --help |
| A063 | lf session serve-flow <task_id> | C015 | Select task id | N053; P:768 | keep → <task_id> |
| A064 | lf session serve-flow <invocation_id> | C015 | Select invocation id | N053; P:769 | keep → <invocation_id> |
| A065 | lf session serve-flow <flow> | C015 | Select flow | N053; P:770 | keep → <flow> |
| A066 | lf session serve-flow <node_id> | C015 | Select node id | N053; P:771 | keep → <node_id> |
| A067 | lf session serve-flow <skill> | C015 | Select skill | N053; P:772 | keep → <skill> |
| A068 | lf session serve-flow <iteration> | C015 | Select iteration | N053; P:773 | keep → <iteration> |
| A069 | lf session serve-flow --help / -h | C015 | Explain this canonical command without performing it | N059; P:767 | keep → --help |
| A070 | lf session serve-ask <id> | C016 | Select id | N053; P:777 | keep → <id> |
| A071 | lf session serve-ask --help / -h | C016 | Explain this canonical command without performing it | N059; P:777 | keep → --help |
| A072 | lf session stop-run <run_id> | C017 | Select run id | N053; P:780 | keep → <run_id> |
| A073 | lf session stop-run --help / -h | C017 | Explain this canonical command without performing it | N059; P:780 | keep → --help |
| A074 | lf install --help / -h | C018 | Explain this canonical command without performing it | N059; P:262 | keep → --help |
| A075 | lf install schedule <frequency> | C019 | Weekly: Monday 09:00; daily: 09:00; otherwise on clock boundaries (local time) | N053; P:1151 | keep → <frequency> |
| A076 | lf install schedule --help / -h | C019 | Explain this canonical command without performing it | N059; P:1148 | keep → --help |
| A077 | lf install recover-switch --switch | C020 | The fixed machine switch receipt to continue | N053; P:1158 | keep → --switch |
| A078 | lf install recover-switch --help / -h | C020 | Explain this canonical command without performing it | N059; P:1155 | keep → --help |
| A079 | lf install preflight --json | C021 | Emit structured evidence for this operation | N060; P:1168 | keep → --json |
| A080 | lf install preflight --help / -h | C021 | Explain this canonical command without performing it | N059; P:1165 | keep → --help |
| A081 | lf install local-preflight --store | C022 | Select store | N053; P:1174 | keep → --store |
| A082 | lf install local-preflight --json | C022 | Emit structured evidence for this operation | N060; P:1176 | keep → --json |
| A083 | lf install local-preflight --help / -h | C022 | Explain this canonical command without performing it | N059; P:1172 | keep → --help |
| A084 | lf install advance-switch --switch | C023 | Select switch | N053; P:1182 | keep → --switch |
| A085 | lf install advance-switch --help / -h | C023 | Explain this canonical command without performing it | N059; P:1180 | keep → --help |
| A086 | lf install promote --from-build | C024 | Promote this exact unpublished local lf into a disposable installed Home | N053; P:1192 | keep → --from-build |
| A087 | lf install promote --coordinated-build (hidden) | C024 | Candidate delegated to the receipt-pinned active coordinator | N061; P:1195 | keep → --coordinated-build |
| A088 | lf install promote --fresh | C024 | Abandon an incompatible disposable Home and fork published data again | N053; P:1198 | keep → --fresh |
| A089 | lf install promote --reuse-home | C024 | Reuse a retained development installation and its existing Home data | N053; P:1201 | keep → --reuse-home |
| A090 | lf install promote --cli-target | C024 | The global CLI symlink to replace (e.g. ~/.local/bin/lf) | N061; P:1204 | keep → --cli-target |
| A091 | lf install promote --daemon-source | C024 | The staged lfd built from the same candidate source | N061; P:1207 | keep → --daemon-source |
| A092 | lf install promote --daemon-target | C024 | The global lfd symlink to replace (e.g. ~/.local/bin/lfd) | N061; P:1210 | keep → --daemon-target |
| A093 | lf install promote --app-source | C024 | A staged Loopflow.app bundle to install alongside the CLI | N061; P:1213 | keep → --app-source |
| A094 | lf install promote --app-target | C024 | The global Loopflow.app path to replace atomically | N061; P:1216 | keep → --app-target |
| A095 | lf install promote --legacy-app-target | C024 | A retired app bundle to remove after the new app commits | N061; P:1219 | keep → --legacy-app-target |
| A096 | lf install promote --sync-skills | C024 | Regenerate global skills after the promotion commits | N053; P:1222 | keep → --sync-skills |
| A097 | lf install promote --preview | C024 | Validate and print the preview but change nothing | N062; P:1225 | keep → --preview |
| A098 | lf install promote --help / -h | C024 | Explain this canonical command without performing it | N059; P:1189 | keep → --help |
| A099 | lf install rollback --cli-target | C025 | The global CLI symlink to replace (e.g. ~/.local/bin/lf) | N061; P:1233 | keep → --cli-target |
| A100 | lf install rollback --candidate | C025 | The immutable content-addressed prior executable to activate | N061; P:1236 | keep → --candidate |
| A101 | lf install rollback --daemon-target | C025 | The global lfd symlink to restore with the CLI | N061; P:1239 | keep → --daemon-target |
| A102 | lf install rollback --daemon-candidate | C025 | The retained immutable lfd binary paired with the CLI candidate | N061; P:1242 | keep → --daemon-candidate |
| A103 | lf install rollback --help / -h | C025 | Explain this canonical command without performing it | N059; P:1230 | keep → --help |
| A104 | lf pr --help / -h | C026 | Explain this canonical command without performing it | N059; P:267 | keep → --help |
| A105 | lf pr checks --watch / -w | C027 | Select watch | N053; P:1251 | keep → --watch |
| A106 | lf pr checks --logs / -l | C027 | Select logs | N053; P:1253 | keep → --logs |
| A107 | lf pr checks --help / -h | C027 | Explain this canonical command without performing it | N059; P:1249 | keep → --help |
| A108 | lf pr status --help / -h | C028 | Explain this canonical command without performing it | N059; P:1257 | keep → --help |
| A109 | lf pr next <slug> | C029 | Name the next serial branch (defaults to the settled PR's next slug, then the sequence number) | N053; P:1263 | keep → <slug> |
| A110 | lf pr next --help / -h | C029 | Explain this canonical command without performing it | N059; P:1260 | keep → --help |
| A111 | lf pr publish --model / -m | C030 | Select model | N053; P:1269 | keep → --model |
| A112 | lf pr publish --title | C030 | Select title | N053; P:1271 | keep → --title |
| A113 | lf pr publish --body | C030 | Select body | N053; P:1273 | keep → --body |
| A114 | lf pr publish --help / -h | C030 | Explain this canonical command without performing it | N059; P:1267 | keep → --help |
| A115 | lf pr open --model / -m | C031 | Select model | N053; P:1279 | keep → --model |
| A116 | lf pr open --title | C031 | Select title | N053; P:1281 | keep → --title |
| A117 | lf pr open --body | C031 | Select body | N053; P:1283 | keep → --body |
| A118 | lf pr open --help / -h | C031 | Explain this canonical command without performing it | N059; P:1277 | keep → --help |
| A119 | lf pr submit --strict | C032 | Select strict | N053; P:1289 | keep → --strict |
| A120 | lf pr submit --create-pr / -p | C032 | Select create pr | N053; P:1291 | keep → --create-pr |
| A121 | lf pr submit --complete / -c | C032 | Select complete | N053; P:1293 | keep → --complete |
| A122 | lf pr submit --next | C032 | Select next | N053; P:1295 | keep → --next |
| A123 | lf pr submit --worktree / -w | C032 | Select worktree | N053; P:1297 | keep → --worktree |
| A124 | lf pr submit --message / -m | C032 | Select message | N053; P:1299 | keep → --message |
| A125 | lf pr submit --title | C032 | Select title | N053; P:1301 | keep → --title |
| A126 | lf pr submit --body | C032 | Select body | N053; P:1303 | keep → --body |
| A127 | lf pr submit --help / -h | C032 | Explain this canonical command without performing it | N059; P:1287 | keep → --help |
| A128 | lf pr arm --strict | C033 | Select strict | N053; P:1308 | keep → --strict |
| A129 | lf pr arm --local | C033 | Select local | N053; P:1310 | keep → --local |
| A130 | lf pr arm --complete / -c | C033 | Select complete | N053; P:1312 | keep → --complete |
| A131 | lf pr arm --next | C033 | Select next | N053; P:1314 | keep → --next |
| A132 | lf pr arm --worktree / -w | C033 | Select worktree | N053; P:1316 | keep → --worktree |
| A133 | lf pr arm --message / -m | C033 | Select message | N053; P:1318 | keep → --message |
| A134 | lf pr arm --title | C033 | Select title | N053; P:1320 | keep → --title |
| A135 | lf pr arm --body | C033 | Select body | N053; P:1322 | keep → --body |
| A136 | lf pr arm --help / -h | C033 | Explain this canonical command without performing it | N059; P:1306 | keep → --help |
| A137 | lf pr land --strict | C034 | Select strict | N053; P:1327 | keep → --strict |
| A138 | lf pr land --local | C034 | Select local | N053; P:1329 | keep → --local |
| A139 | lf pr land --complete / -c | C034 | Select complete | N053; P:1331 | keep → --complete |
| A140 | lf pr land --next | C034 | Select next | N053; P:1333 | keep → --next |
| A141 | lf pr land --worktree / -w | C034 | Select worktree | N053; P:1335 | keep → --worktree |
| A142 | lf pr land --message / -m | C034 | Select message | N053; P:1337 | keep → --message |
| A143 | lf pr land --title | C034 | Select title | N053; P:1339 | keep → --title |
| A144 | lf pr land --body | C034 | Select body | N053; P:1341 | keep → --body |
| A145 | lf pr land --help / -h | C034 | Explain this canonical command without performing it | N059; P:1325 | keep → --help |
| A146 | lf pr abandon <branch> | C035 | Branch to abandon (default: current) | N053; P:1346 | keep → <branch> |
| A147 | lf pr abandon --force / -f | C035 | Select force | N055; P:1348 | keep → --force |
| A148 | lf pr abandon --help / -h | C035 | Explain this canonical command without performing it | N059; P:1344 | keep → --help |
| A149 | lf wt --help / -h | C036 | Explain this canonical command without performing it | N059; P:272 | keep → --help |
| A150 | lf wt create <name> | C037 | Worktree name | N053; P:1640 | keep → <name> |
| A151 | lf wt create --plan | C037 | Print the placement plan without creating a worktree | N062; P:1643 | keep → --plan |
| A152 | lf wt create --help / -h | C037 | Explain this canonical command without performing it | N059; P:1638 | keep → --help |
| A153 | lf wt switch <name> | C038 | Worktree name or full branch name to switch to | N053; P:1648 | keep → <name> |
| A154 | lf wt switch --help / -h | C038 | Explain this canonical command without performing it | N059; P:1646 | keep → --help |
| A155 | lf wt list --format | C039 | Select format | N063; P:1653 | rename → --json |
| A156 | lf wt list --full | C039 | Select full | N064; P:1655 | delete → — |
| A157 | lf wt list --sync | C039 | Fetch origin and fast-forward main before listing (mutates the canonical checkout). Off by default so a list never touches it | N053; P:1659 | keep → --sync |
| A158 | lf wt list --help / -h | C039 | Explain this canonical command without performing it | N059; P:1651 | keep → --help |
| A159 | lf wt prune --dry-run | C040 | Show what would be pruned without removing anything | N062; P:1665 | keep → --dry-run |
| A160 | lf wt prune --help / -h | C040 | Explain this canonical command without performing it | N059; P:1662 | keep → --help |
| A161 | lf wt remove <name> | C041 | Worktree name to remove | N053; P:1671 | keep → <name> |
| A162 | lf wt remove --force / -f | C041 | Select force | N055; P:1673 | keep → --force |
| A163 | lf wt remove --help / -h | C041 | Explain this canonical command without performing it | N059; P:1669 | keep → --help |
| A164 | lf rebase --plan | C042 | Print the planned rebase strategy without mutating git | N062; P:280 | keep → --plan |
| A165 | lf rebase --manual | C042 | Keep the rebase local and leave conflicts for this process to resolve | N053; P:283 | keep → --manual |
| A166 | lf rebase --continue | C042 | Stage resolved conflict paths and continue the local rebase | N053; P:286 | keep → --continue |
| A167 | lf rebase --abort | C042 | Abort the local rebase in progress | N053; P:289 | keep → --abort |
| A168 | lf rebase --adopt | C042 | Explicitly claim a raw rebase that has no Loopflow owner | N053; P:292 | keep → --adopt |
| A169 | lf rebase <onto> | C042 | Branch to rebase onto | N053; P:294 | keep → <onto> |
| A170 | lf rebase --help / -h | C042 | Explain this canonical command without performing it | N059; P:277 | keep → --help |
| A171 | lf commit --message / -m | C043 | Select message | N053; P:299 | keep → --message |
| A172 | lf commit --push / -p | C043 | Select push | N065; P:301 | merge into → lf task pr open |
| A173 | lf commit --no-add | C043 | Select no add | N053; P:303 | keep → --no-add |
| A174 | lf commit --help / -h | C043 | Explain this canonical command without performing it | N059; P:297 | keep → --help |
| A175 | lf auth --help / -h | C044 | Explain this canonical command without performing it | N059; P:306 | keep → --help |
| A176 | lf auth status <provider> | C045 | Select provider | N053; P:1494 | keep → <provider> |
| A177 | lf auth status --verify | C045 | Select verify | N053; P:1496 | keep → --verify |
| A178 | lf auth status --details | C045 | Select details | N053; P:1498 | keep → --details |
| A179 | lf auth status --json | C045 | Emit structured evidence for this operation | N060; P:1500 | keep → --json |
| A180 | lf auth status --help / -h | C045 | Explain this canonical command without performing it | N059; P:1493 | keep → --help |
| A181 | lf auth disconnect <provider> | C046 | Select provider | N053; P:1504 | keep → <provider> |
| A182 | lf auth disconnect <email> | C046 | Select email | N053; P:1505 | keep → <email> |
| A183 | lf auth disconnect --help / -h | C046 | Explain this canonical command without performing it | N059; P:1503 | keep → --help |
| A184 | lf auth connect <provider> | C047 | Select provider | N053; P:1509 | keep → <provider> |
| A185 | lf auth connect <email> | C047 | Select email | N053; P:1510 | keep → <email> |
| A186 | lf auth connect --chrome-profile | C047 | Select chrome profile | N053; P:1512 | keep → --chrome-profile |
| A187 | lf auth connect --import | C047 | Adopt an existing Claude login | N053; P:1515 | keep → --import |
| A188 | lf auth connect --api-key | C047 | Read the provider's API key environment variable | N053; P:1518 | keep → --api-key |
| A189 | lf auth connect --help / -h | C047 | Explain this canonical command without performing it | N059; P:1508 | keep → --help |
| A190 | lf auth set <provider> | C048 | Select provider | N053; P:1522 | keep → <provider> |
| A191 | lf auth set <email> | C048 | Select email | N053; P:1523 | keep → <email> |
| A192 | lf auth set --login-email | C048 | Select login email | N053; P:1525 | keep → --login-email |
| A193 | lf auth set --routing | C048 | Select routing | N053; P:1527 | keep → --routing |
| A194 | lf auth set --plan | C048 | Select plan | N062; P:1529 | keep → --plan |
| A195 | lf auth set --clear-plan | C048 | Select clear plan | N053; P:1531 | keep → --clear-plan |
| A196 | lf auth set --paid-through | C048 | Select paid through | N053; P:1533 | keep → --paid-through |
| A197 | lf auth set --clear-paid-through | C048 | Select clear paid through | N053; P:1535 | keep → --clear-paid-through |
| A198 | lf auth set --clear-cooldown | C048 | Select clear cooldown | N053; P:1537 | keep → --clear-cooldown |
| A199 | lf auth set --chrome-profile | C048 | Replace the ordered browser choices (repeat for fallback profiles) | N053; P:1540 | keep → --chrome-profile |
| A200 | lf auth set --clear-chrome-profiles | C048 | Select clear chrome profiles | N053; P:1542 | keep → --clear-chrome-profiles |
| A201 | lf auth set --help / -h | C048 | Explain this canonical command without performing it | N059; P:1521 | keep → --help |
| A202 | lf auth route --help / -h | C049 | Explain this canonical command without performing it | N059; P:1545 | keep → --help |
| A203 | lf auth route set <provider> | C050 | Select provider | N053; P:1555 | keep → <provider> |
| A204 | lf auth route set <accounts> | C050 | Select accounts | N053; P:1557 | keep → <accounts> |
| A205 | lf auth route set --repo | C050 | Select repo | N053; P:1559 | keep → --repo |
| A206 | lf auth route set --default | C050 | Select default | N053; P:1561 | keep → --default |
| A207 | lf auth route set --help / -h | C050 | Explain this canonical command without performing it | N059; P:1554 | keep → --help |
| A208 | lf auth route show --repo | C051 | Select repo | N053; P:1566 | keep → --repo |
| A209 | lf auth route show --default | C051 | Select default | N053; P:1568 | keep → --default |
| A210 | lf auth route show --json | C051 | Emit structured evidence for this operation | N060; P:1570 | keep → --json |
| A211 | lf auth route show --help / -h | C051 | Explain this canonical command without performing it | N059; P:1564 | keep → --help |
| A212 | lf release --help / -h | C052 | Explain this canonical command without performing it | N059; P:311 | keep → --help |
| A213 | lf release run <version> | C053 | Version to release: patch\|minor\|major\|X.Y.Z (default: patch) | N053; P:1579 | keep → <version> |
| A214 | lf release run --target / -t | C053 | Select target | N053; P:1581 | keep → --target |
| A215 | lf release run --help / -h | C053 | Explain this canonical command without performing it | N059; P:1577 | keep → --help |
| A216 | lf release check --target / -t | C054 | Select target | N053; P:1586 | keep → --target |
| A217 | lf release check --help / -h | C054 | Explain this canonical command without performing it | N059; P:1584 | keep → --help |
| A218 | lf release notes <version> | C055 | Version (e.g. 0.9.6) | N053; P:1591 | keep → <version> |
| A219 | lf release notes --prev-tag | C055 | Select prev tag | N053; P:1593 | keep → --prev-tag |
| A220 | lf release notes --preview | C055 | Print notes without updating manifests or release archives | N062; P:1596 | keep → --preview |
| A221 | lf release notes --target / -t | C055 | Select target | N053; P:1598 | keep → --target |
| A222 | lf release notes --help / -h | C055 | Explain this canonical command without performing it | N059; P:1589 | keep → --help |
| A223 | lf release bump <version> | C056 | Version to bump to (e.g. 0.9.6) | N053; P:1603 | keep → <version> |
| A224 | lf release bump --target / -t | C056 | Select target | N053; P:1605 | keep → --target |
| A225 | lf release bump --help / -h | C056 | Explain this canonical command without performing it | N059; P:1601 | keep → --help |
| A226 | lf release tag <version> | C057 | Version to tag (e.g. 0.9.6) | N053; P:1610 | keep → <version> |
| A227 | lf release tag --target / -t | C057 | Select target | N053; P:1612 | keep → --target |
| A228 | lf release tag --help / -h | C057 | Explain this canonical command without performing it | N059; P:1608 | keep → --help |
| A229 | lf release publish <tag> | C058 | Release tag (for example v0.12.4) | N053; P:1617 | keep → <tag> |
| A230 | lf release publish --notes | C058 | Release notes used while creating or updating the draft | N053; P:1620 | keep → --notes |
| A231 | lf release publish --asset | C058 | Asset to upload; repeat for multiple files | N053; P:1623 | keep → --asset |
| A232 | lf release publish --finalize | C058 | Publish the existing draft and mark it latest | N053; P:1626 | keep → --finalize |
| A233 | lf release publish --help / -h | C058 | Explain this canonical command without performing it | N059; P:1615 | keep → --help |
| A234 | lf release status --target / -t | C059 | Select target | N053; P:1631 | keep → --target |
| A235 | lf release status --help / -h | C059 | Explain this canonical command without performing it | N059; P:1629 | keep → --help |
| A236 | lf repo --help / -h | C060 | Explain this canonical command without performing it | N059; P:316 | keep → --help |
| A237 | lf repo reteam --apply | C061 | Select apply | N053; P:1446 | keep → --apply |
| A238 | lf repo reteam --help / -h | C061 | Explain this canonical command without performing it | N059; P:1444 | keep → --help |
| A239 | lf repo webhook --help / -h | C062 | Explain this canonical command without performing it | N059; P:1449 | keep → --help |
| A240 | lf repo webhook serve --addr | C063 | Address to bind (a reverse proxy gives Linear the public HTTPS URL) | N053; P:1462 | keep → --addr |
| A241 | lf repo webhook serve --help / -h | C063 | Explain this canonical command without performing it | N059; P:1459 | keep → --help |
| A242 | lf repo webhook register --url | C064 | Public HTTPS URL Linear will POST deliveries to | N053; P:1469 | keep → --url |
| A243 | lf repo webhook register --help / -h | C064 | Explain this canonical command without performing it | N059; P:1466 | keep → --help |
| A244 | lf home --help / -h | C065 | Explain this canonical command without performing it | N059; P:321 | keep → --help |
| A245 | lf home id --json | C066 | Emit structured evidence for this operation | N060; P:1479 | keep → --json |
| A246 | lf home id --help / -h | C066 | Explain this canonical command without performing it | N059; P:1477 | keep → --help |
| A247 | lf home observe <home_id> | C067 | Select home id | N053; P:1483 | keep → <home_id> |
| A248 | lf home observe <route> | C067 | Select route | N053; P:1484 | keep → <route> |
| A249 | lf home observe --json | C067 | Emit structured evidence for this operation | N060; P:1486 | keep → --json |
| A250 | lf home observe --help / -h | C067 | Explain this canonical command without performing it | N059; P:1482 | keep → --help |
| A251 | lf sync-skills --yes / -y | C068 | Confirm writes under ~/ without prompting | N055; P:330 | keep → --yes |
| A252 | lf sync-skills --no-prune | C068 | Keep stale loopflow-generated skills | N053; P:333 | keep → --no-prune |
| A253 | lf sync-skills --help / -h | C068 | Explain this canonical command without performing it | N059; P:327 | keep → --help |
| A254 | lf cron --help / -h | C069 | Explain this canonical command without performing it | N059; P:336 | keep → --help |
| A255 | lf cron add --wave / -w | C070 | Wave name passed to `lf <flow> --wave <wave>` (ambient if omitted) | N058; P:1358 | keep → --wave |
| A256 | lf cron add --flow | C070 | Flow or skill name to run | N053; P:1361 | keep → --flow |
| A257 | lf cron add --schedule | C070 | Fixed-daily cron expression, or the `daily` alias | N053; P:1364 | keep → --schedule |
| A258 | lf cron add --help / -h | C070 | Explain this canonical command without performing it | N059; P:1355 | keep → --help |
| A259 | lf cron list --wave / -w | C071 | Only jobs for this Wave | N058; P:1370 | keep → --wave |
| A260 | lf cron list --json | C071 | Emit structured evidence for this operation | N060; P:1373 | keep → --json |
| A261 | lf cron list --help / -h | C071 | Explain this canonical command without performing it | N059; P:1367 | keep → --help |
| A262 | lf cron preflight --wave / -w | C072 | Wave whose GOAL.md `crons:` are validated | N058; P:1379 | keep → --wave |
| A263 | lf cron preflight --help / -h | C072 | Explain this canonical command without performing it | N059; P:1376 | keep → --help |
| A264 | lf cron sync --wave / -w | C073 | Wave whose GOAL.md `crons:` drive the installed jobs | N058; P:1385 | keep → --wave |
| A265 | lf cron sync --help / -h | C073 | Explain this canonical command without performing it | N059; P:1382 | keep → --help |
| A266 | lf cron run --wave / -w | C074 | Wave whose installed declaration is executed | N058; P:1392 | keep → --wave |
| A267 | lf cron run --flow | C074 | Flow or skill name to run | N053; P:1395 | keep → --flow |
| A268 | lf cron run --scheduled (hidden) | C074 | Mark a launchd-owned invocation | N053; P:1398 | keep → --scheduled |
| A269 | lf cron run --help / -h | C074 | Explain this canonical command without performing it | N059; P:1389 | keep → --help |
| A270 | lf cron history --wave / -w | C075 | Wave whose receipts are shown | N058; P:1404 | keep → --wave |
| A271 | lf cron history --flow | C075 | Only receipts for this flow or skill | N053; P:1407 | keep → --flow |
| A272 | lf cron history --days | C075 | Receipt window in days | N053; P:1410 | keep → --days |
| A273 | lf cron history --json | C075 | Emit structured evidence for this operation | N060; P:1413 | keep → --json |
| A274 | lf cron history --help / -h | C075 | Explain this canonical command without performing it | N059; P:1401 | keep → --help |
| A275 | lf cron trigger --wave / -w | C076 | Wave whose installed job is fired | N058; P:1419 | keep → --wave |
| A276 | lf cron trigger --flow | C076 | Flow or skill name to run | N053; P:1422 | keep → --flow |
| A277 | lf cron trigger --wait | C076 | Wait for and return the scheduled receipt | N053; P:1425 | keep → --wait |
| A278 | lf cron trigger --timeout | C076 | Maximum wait for a receipt | N053; P:1428 | keep → --timeout |
| A279 | lf cron trigger --help / -h | C076 | Explain this canonical command without performing it | N059; P:1416 | keep → --help |
| A280 | lf cron remove --wave / -w | C077 | Wave name passed to `lf <flow> --wave <wave>` | N058; P:1434 | keep → --wave |
| A281 | lf cron remove --flow | C077 | Flow or skill name to remove | N053; P:1437 | keep → --flow |
| A282 | lf cron remove --help / -h | C077 | Explain this canonical command without performing it | N059; P:1431 | keep → --help |
| A283 | lf wave --help / -h | C078 | Explain this canonical command without performing it | N059; P:341 | keep → --help |
| A284 | lf wave list --json | C079 | Emit structured evidence for this operation | N060; P:827 | keep → --json |
| A285 | lf wave list --all | C079 | List Waves from every repository on this machine, not just the current repository (worktrees collapse to their main checkout) | N053; P:831 | keep → --all |
| A286 | lf wave list --current | C079 | Exclude abandoned and retired registrations from current navigation | N053; P:834 | keep → --current |
| A287 | lf wave list --help / -h | C079 | Explain this canonical command without performing it | N059; P:824 | keep → --help |
| A288 | lf wave status <wave> | C080 | Wave name (default: the ambient wave) | N058; P:840 | keep → <wave> |
| A289 | lf wave status --chapter | C080 | Read a dated chapter snapshot instead of current execution | N053; P:843 | keep → --chapter |
| A290 | lf wave status --json | C080 | Emit structured evidence for this operation | N060; P:846 | keep → --json |
| A291 | lf wave status --sync | C080 | Refresh planning from Linear before reading | N053; P:849 | keep → --sync |
| A292 | lf wave status --no-sync | C080 | Read cached planning | N066; P:852 | delete → — |
| A293 | lf wave status --help / -h | C080 | Explain this canonical command without performing it | N059; P:838 | keep → --help |
| A294 | lf wave probe <wave> | C081 | Wave name; defaults to the ambient wave | N058; P:861 | keep → <wave> |
| A295 | lf wave probe --json | C081 | Emit structured evidence for this operation | N060; P:863 | keep → --json |
| A296 | lf wave probe --help / -h | C081 | Explain this canonical command without performing it | N059; P:859 | keep → --help |
| A297 | lf wave connect <wave> | C082 | Wave name (auto-detected if omitted) | N058; P:869 | keep → <wave> |
| A298 | lf wave connect --wave / -w | C082 | Wave name (flag form; same as positional wave) | N053; P:872 | keep → --wave |
| A299 | lf wave connect --all | C082 | Recursively initialize every Wave under wave/ | N053; P:875 | keep → --all |
| A300 | lf wave connect --team-key | C082 | Repository Team key = Task prefix (e.g. LOO). Defaults from the repository name | N053; P:878 | keep → --team-key |
| A301 | lf wave connect --team-name | C082 | Repository Team display name. Defaults to the repository name | N053; P:881 | keep → --team-name |
| A302 | lf wave connect --help / -h | C082 | Explain this canonical command without performing it | N059; P:867 | keep → --help |
| A303 | lf wave sync <wave> | C083 | Select wave | N058; P:885 | keep → <wave> |
| A304 | lf wave sync --wave / -w | C083 | Select wave flag | N053; P:887 | keep → --wave |
| A305 | lf wave sync --all | C083 | Select all | N053; P:889 | keep → --all |
| A306 | lf wave sync --help / -h | C083 | Explain this canonical command without performing it | N059; P:884 | keep → --help |
| A307 | lf wave rename <wave> | C084 | Select wave | N058; P:893 | keep → <wave> |
| A308 | lf wave rename --title | C084 | Select title | N053; P:895 | keep → --title |
| A309 | lf wave rename --help / -h | C084 | Explain this canonical command without performing it | N059; P:892 | keep → --help |
| A310 | lf wave forget <name> | C085 | Select name | N053; P:899 | keep → <name> |
| A311 | lf wave forget --dry-run | C085 | Select dry run | N062; P:901 | keep → --dry-run |
| A312 | lf wave forget --json | C085 | Emit structured evidence for this operation | N060; P:903 | keep → --json |
| A313 | lf wave forget --help / -h | C085 | Explain this canonical command without performing it | N059; P:898 | keep → --help |
| A314 | lf wave place <name> | C086 | Select name | N053; P:907 | keep → <name> |
| A315 | lf wave place <home_id> | C086 | Select home id | N053; P:908 | keep → <home_id> |
| A316 | lf wave place --json | C086 | Emit structured evidence for this operation | N060; P:910 | keep → --json |
| A317 | lf wave place --help / -h | C086 | Explain this canonical command without performing it | N059; P:906 | keep → --help |
| A318 | lf wave relocate <wave> | C087 | Select wave | N058; P:914 | keep → <wave> |
| A319 | lf wave relocate --repo | C087 | Select repo | N053; P:916 | keep → --repo |
| A320 | lf wave relocate --name | C087 | Select name | N053; P:918 | keep → --name |
| A321 | lf wave relocate --json | C087 | Emit structured evidence for this operation | N060; P:920 | keep → --json |
| A322 | lf wave relocate --help / -h | C087 | Explain this canonical command without performing it | N059; P:913 | keep → --help |
| A323 | lf wave retire <name> | C088 | Select name | N053; P:924 | keep → <name> |
| A324 | lf wave retire --reason | C088 | Select reason | N053; P:926 | keep → --reason |
| A325 | lf wave retire --json | C088 | Emit structured evidence for this operation | N060; P:928 | keep → --json |
| A326 | lf wave retire --help / -h | C088 | Explain this canonical command without performing it | N059; P:923 | keep → --help |
| A327 | lf wave recover <name> | C089 | Select name | N053; P:933 | keep → <name> |
| A328 | lf wave recover --cancel | C089 | Cancel exactly this journal source sequence without executing its work | N053; P:936 | keep → --cancel |
| A329 | lf wave recover --reason | C089 | Select reason | N053; P:938 | keep → --reason |
| A330 | lf wave recover --help / -h | C089 | Explain this canonical command without performing it | N059; P:932 | keep → --help |
| A331 | lf wave new-chapter --wave / -w | C090 | Select wave | N058; P:943 | keep → --wave |
| A332 | lf wave new-chapter --chapter | C090 | Select chapter | N053; P:945 | keep → --chapter |
| A333 | lf wave new-chapter --plan | C090 | Fresh chapter content as JSON: metric_targets, flows, and krs | N062; P:948 | keep → --plan |
| A334 | lf wave new-chapter --dry-run | C090 | Select dry run | N062; P:950 | keep → --dry-run |
| A335 | lf wave new-chapter --json | C090 | Emit structured evidence for this operation | N060; P:952 | keep → --json |
| A336 | lf wave new-chapter --help / -h | C090 | Explain this canonical command without performing it | N059; P:941 | keep → --help |
| A337 | lf wave history --wave / -w | C091 | Select wave | N058; P:957 | keep → --wave |
| A338 | lf wave history --json | C091 | Emit structured evidence for this operation | N060; P:959 | keep → --json |
| A339 | lf wave history --help / -h | C091 | Explain this canonical command without performing it | N059; P:955 | keep → --help |
| A340 | lf wave update-plan --wave / -w | C092 | Select wave | N058; P:964 | keep → --wave |
| A341 | lf wave update-plan --plan | C092 | Select plan | N062; P:966 | keep → --plan |
| A342 | lf wave update-plan --help / -h | C092 | Explain this canonical command without performing it | N059; P:962 | keep → --help |
| A343 | lf __chat-connect <waves> | C093 | Wave names. With none, starts eligible Waves in the current repo | N053; P:349 | keep → <waves> |
| A344 | lf __chat-connect --wave-id (hidden) | C093 | Internal identity bindings carried by an explicit Home SSH hop | N053; P:352 | keep → --wave-id |
| A345 | lf __chat-connect --json | C093 | Emit structured evidence for this operation | N060; P:354 | keep → --json |
| A346 | lf __chat-connect --help / -h | C093 | Explain this canonical command without performing it | N059; P:347 | keep → --help |
| A347 | lf __resident <name> | C094 | Wave name | N053; P:361 | keep → <name> |
| A348 | lf __resident --help / -h | C094 | Explain this canonical command without performing it | N059; P:359 | keep → --help |
| A349 | lf task --help / -h | C095 | Explain this canonical command without performing it | N059; P:365 | keep → --help |
| A350 | lf task __worker <task_id> | C096 | Select task id | N053; P:974 | keep → <task_id> |
| A351 | lf task __worker --help / -h | C096 | Explain this canonical command without performing it | N059; P:974 | keep → --help |
| A352 | lf task checkout <issue> | C097 | Select issue | N053; P:977 | keep → <issue> |
| A353 | lf task checkout --name | C097 | Select name | N053; P:979 | keep → --name |
| A354 | lf task checkout --stack-on | C097 | Fork this Task's worktree from another Task's active PR | N053; P:982 | keep → --stack-on |
| A355 | lf task checkout --directive | C097 | Select directive | N053; P:984 | keep → --directive |
| A356 | lf task checkout --json | C097 | Emit structured evidence for this operation | N060; P:986 | keep → --json |
| A357 | lf task checkout --help / -h | C097 | Explain this canonical command without performing it | N059; P:976 | keep → --help |
| A358 | lf task run <issue> | C098 | Select issue | N053; P:990 | keep → <issue> |
| A359 | lf task run --name | C098 | Select name | N053; P:992 | keep → --name |
| A360 | lf task run --flow | C098 | Select a Flow for this Task worker; defaults to the chapter recommendation | N053; P:995 | keep → --flow |
| A361 | lf task run --stack-on | C098 | Fork this Task's worktree from another Task's active PR | N053; P:998 | keep → --stack-on |
| A362 | lf task run --directive | C098 | Select directive | N053; P:1000 | keep → --directive |
| A363 | lf task run --reason | C098 | Explain what changed after an execution blocker | N053; P:1003 | keep → --reason |
| A364 | lf task run --json | C098 | Emit structured evidence for this operation | N060; P:1005 | keep → --json |
| A365 | lf task run --help / -h | C098 | Explain this canonical command without performing it | N059; P:989 | keep → --help |
| A366 | lf task create --wave | C099 | Wave name; defaults to the bound Wave | N058; P:1011 | keep → --wave |
| A367 | lf task create --title | C099 | Task title; omitted when stdin supplies the report and first line | N053; P:1014 | keep → --title |
| A368 | lf task create --notes | C099 | Description; defaults to a report read from stdin | N053; P:1017 | keep → --notes |
| A369 | lf task create --run | C099 | Validate placement and execution before filing, then run the Task | N053; P:1020 | keep → --run |
| A370 | lf task create --name | C099 | Select name | N053; P:1022 | keep → --name |
| A371 | lf task create --flow | C099 | Select a Flow for this Task worker; defaults to the chapter recommendation | N053; P:1025 | keep → --flow |
| A372 | lf task create --stack-on | C099 | Fork this Task's worktree from another Task's active PR | N053; P:1028 | keep → --stack-on |
| A373 | lf task create --json | C099 | Emit structured evidence for this operation | N060; P:1030 | keep → --json |
| A374 | lf task create --help / -h | C099 | Explain this canonical command without performing it | N059; P:1008 | keep → --help |
| A375 | lf task status <issue> | C100 | Task issue; defaults to the Task in this checkout | N053; P:1035 | keep → <issue> |
| A376 | lf task status --json | C100 | Emit structured evidence for this operation | N060; P:1037 | keep → --json |
| A377 | lf task status --help / -h | C100 | Explain this canonical command without performing it | N059; P:1033 | keep → --help |
| A378 | lf task changes <issue> | C101 | Select issue | N053; P:1041 | keep → <issue> |
| A379 | lf task changes --base | C101 | Select base | N053; P:1043 | keep → --base |
| A380 | lf task changes --json | C101 | Emit structured evidence for this operation | N060; P:1045 | keep → --json |
| A381 | lf task changes --help / -h | C101 | Explain this canonical command without performing it | N059; P:1040 | keep → --help |
| A382 | lf task diff <issue> | C102 | Select issue | N053; P:1049 | keep → <issue> |
| A383 | lf task diff <path> | C102 | Select path | N053; P:1050 | keep → <path> |
| A384 | lf task diff --base | C102 | Select base | N053; P:1052 | keep → --base |
| A385 | lf task diff --draft | C102 | Compare a UTF-8 draft read from stdin without writing the worktree | N053; P:1055 | keep → --draft |
| A386 | lf task diff --json | C102 | Emit structured evidence for this operation | N060; P:1057 | keep → --json |
| A387 | lf task diff --help / -h | C102 | Explain this canonical command without performing it | N059; P:1048 | keep → --help |
| A388 | lf task file <issue> | C103 | Select issue | N053; P:1061 | keep → <issue> |
| A389 | lf task file <path> | C103 | Select path | N053; P:1062 | keep → <path> |
| A390 | lf task file --recoveries | C103 | Inspect retained versions, including late writes; omitted for fast content reads | N053; P:1065 | keep → --recoveries |
| A391 | lf task file --json | C103 | Emit structured evidence for this operation | N060; P:1067 | keep → --json |
| A392 | lf task file --help / -h | C103 | Explain this canonical command without performing it | N059; P:1060 | keep → --help |
| A393 | lf task save <issue> | C104 | Select issue | N053; P:1071 | keep → <issue> |
| A394 | lf task save <path> | C104 | Select path | N053; P:1072 | keep → <path> |
| A395 | lf task save --revision | C104 | Select revision | N053; P:1074 | keep → --revision |
| A396 | lf task save --json | C104 | Emit structured evidence for this operation | N060; P:1076 | keep → --json |
| A397 | lf task save --help / -h | C104 | Explain this canonical command without performing it | N059; P:1070 | keep → --help |
| A398 | lf task complete <issue> | C105 | Select issue | N053; P:1080 | keep → <issue> |
| A399 | lf task complete --summary | C105 | Select summary | N053; P:1082 | keep → --summary |
| A400 | lf task complete --json | C105 | Emit structured evidence for this operation | N060; P:1084 | keep → --json |
| A401 | lf task complete --help / -h | C105 | Explain this canonical command without performing it | N059; P:1079 | keep → --help |
| A402 | lf task delete <issue> | C106 | Select issue | N053; P:1087 | keep → <issue> |
| A403 | lf task delete --help / -h | C106 | Explain this canonical command without performing it | N059; P:1087 | keep → --help |
| A404 | lf task edit <issue> | C107 | Select issue | N053; P:1090 | keep → <issue> |
| A405 | lf task edit --title | C107 | Select title | N053; P:1092 | keep → --title |
| A406 | lf task edit --notes | C107 | Select notes | N053; P:1094 | keep → --notes |
| A407 | lf task edit --wave / -w | C107 | Select wave | N058; P:1096 | keep → --wave |
| A408 | lf task edit --help / -h | C107 | Explain this canonical command without performing it | N059; P:1089 | keep → --help |
| A409 | lf task comment <issue> | C108 | Select issue | N053; P:1100 | keep → <issue> |
| A410 | lf task comment <message> | C108 | Select message | N053; P:1101 | keep → <message> |
| A411 | lf task comment --wave / -w | C108 | Select wave | N058; P:1103 | keep → --wave |
| A412 | lf task comment --json | C108 | Emit structured evidence for this operation | N060; P:1105 | keep → --json |
| A413 | lf task comment --help / -h | C108 | Explain this canonical command without performing it | N059; P:1099 | keep → --help |
| A414 | lf task interrupt <issue> | C109 | Select issue | N053; P:1109 | keep → <issue> |
| A415 | lf task interrupt --json | C109 | Emit structured evidence for this operation | N060; P:1111 | keep → --json |
| A416 | lf task interrupt --help / -h | C109 | Explain this canonical command without performing it | N059; P:1108 | keep → --help |
| A417 | lf task wait <issue> | C110 | Select issue | N053; P:1115 | keep → <issue> |
| A418 | lf task wait --until | C110 | Select until | N053; P:1117 | keep → --until |
| A419 | lf task wait --timeout | C110 | Select timeout | N053; P:1119 | keep → --timeout |
| A420 | lf task wait --json | C110 | Emit structured evidence for this operation | N060; P:1121 | keep → --json |
| A421 | lf task wait --help / -h | C110 | Explain this canonical command without performing it | N059; P:1114 | keep → --help |
| A422 | lf task restart <issue> | C111 | Select issue | N053; P:1126 | keep → <issue> |
| A423 | lf task restart <advice> | C111 | Select advice | N053; P:1127 | keep → <advice> |
| A424 | lf task restart --flow | C111 | Replacement Flow; validated before any checkpoint or stop | N053; P:1130 | keep → --flow |
| A425 | lf task restart --json | C111 | Emit structured evidence for this operation | N060; P:1132 | keep → --json |
| A426 | lf task restart --help / -h | C111 | Explain this canonical command without performing it | N059; P:1125 | keep → --help |
| A427 | lf tokens --json | C112 | Emit structured evidence for this operation | N060; P:373 | keep → --json |
| A428 | lf tokens --days | C112 | Walk git history instead: the codebase's size on each day it changed | N053; P:376 | keep → --days |
| A429 | lf tokens --help / -h | C112 | Explain this canonical command without performing it | N059; P:370 | keep → --help |
| A430 | lf usage --json | C113 | Emit structured evidence for this operation | N060; P:382 | keep → --json |
| A431 | lf usage --days | C113 | Run window, in days (zero means all time) | N053; P:385 | keep → --days |
| A432 | lf usage --wave | C113 | Limit to Runs attributed to one Wave | N058; P:388 | keep → --wave |
| A433 | lf usage --project | C113 | Limit to Runs attributed to one Project | N058; P:391 | keep → --project |
| A434 | lf usage --task | C113 | Limit to Runs attributed to one Task | N058; P:394 | keep → --task |
| A435 | lf usage --help / -h | C113 | Explain this canonical command without performing it | N059; P:379 | keep → --help |
| A436 | lf __telemetry-scorecard --json | C114 | Emit structured evidence for this operation | N060; P:401 | keep → --json |
| A437 | lf __telemetry-scorecard --help / -h | C114 | Explain this canonical command without performing it | N059; P:398 | keep → --help |
| A438 | lf ci --since | C115 | Relative window (7d, 24h, 30m) or RFC3339 start | N053; P:407 | keep → --since |
| A439 | lf ci --wave | C115 | Scope to one Wave | N058; P:410 | keep → --wave |
| A440 | lf ci --repo | C115 | Scope to one GitHub owner/repo | N053; P:413 | keep → --repo |
| A441 | lf ci --json | C115 | Emit structured evidence for this operation | N060; P:416 | keep → --json |
| A442 | lf ci --help / -h | C115 | Explain this canonical command without performing it | N059; P:404 | keep → --help |
| A443 | lf ps --json | C116 | Emit structured evidence for this operation | N060; P:422 | keep → --json |
| A444 | lf ps --help / -h | C116 | Explain this canonical command without performing it | N059; P:419 | keep → --help |
| A445 | lf top --json | C117 | Emit structured evidence for this operation | N060; P:428 | keep → --json |
| A446 | lf top --help / -h | C117 | Explain this canonical command without performing it | N059; P:425 | keep → --help |
| A447 | lf prune --dry-run | C118 | Show exact targets without changing process or receipt state | N062; P:434 | keep → --dry-run |
| A448 | lf prune --json | C118 | Emit structured evidence for this operation | N060; P:437 | keep → --json |
| A449 | lf prune --help / -h | C118 | Explain this canonical command without performing it | N059; P:431 | keep → --help |
| A450 | lf doctor --planning | C119 | Diagnose repository planning without changing it | N053; P:443 | keep → --planning |
| A451 | lf doctor --json | C119 | Emit structured evidence for this operation | N060; P:446 | keep → --json |
| A452 | lf doctor --help / -h | C119 | Explain this canonical command without performing it | N059; P:440 | keep → --help |
| A453 | lf list <path> | C120 | Select path | N053; P:450 | keep → <path> |
| A454 | lf list --json | C120 | Emit structured evidence for this operation | N060; P:452 | keep → --json |
| A455 | lf list --help / -h | C120 | Explain this canonical command without performing it | N059; P:449 | keep → --help |
| A456 | lf help <path> | C121 | Select path | N053; P:456 | keep → <path> |
| A457 | lf help --all | C121 | Select all | N053; P:458 | keep → --all |
| A458 | lf help --help / -h | C121 | Explain this canonical command without performing it | N059; P:455 | keep → --help |
| A459 | lf roadmap --wave | C122 | Scope to one Wave (default: every Wave in the current repository) | N058; P:467 | keep → --wave |
| A460 | lf roadmap --json | C122 | Emit structured evidence for this operation | N060; P:470 | keep → --json |
| A461 | lf roadmap --all | C122 | Span every repository on this machine, not just the current one | N053; P:473 | keep → --all |
| A462 | lf roadmap --help / -h | C122 | Explain this canonical command without performing it | N059; P:464 | keep → --help |
| A463 | lf activity --since | C123 | Relative window (7d, 24h, 30m) or RFC3339 start | N053; P:479 | keep → --since |
| A464 | lf activity --limit | C123 | Maximum rows after Work filters (1-200) | N053; P:482 | keep → --limit |
| A465 | lf activity --wave | C123 | Scope to one Wave by name | N058; P:485 | keep → --wave |
| A466 | lf activity --project | C123 | Scope to one Project by slug | N058; P:488 | keep → --project |
| A467 | lf activity --task | C123 | Scope to one Task by Linear identifier | N058; P:491 | keep → --task |
| A468 | lf activity --json | C123 | Emit structured evidence for this operation | N060; P:494 | keep → --json |
| A469 | lf activity --help / -h | C123 | Explain this canonical command without performing it | N059; P:476 | keep → --help |
| A470 | lf runs --active | C124 | Observe current provider-backed Runs without the history window or cap | N067; P:500 | rename → lf monitor active |
| A471 | lf runs --watch | C124 | Retain discovery and stream active snapshots until stdin closes | N068; P:503 | rename → lf monitor active --watch |
| A472 | lf runs <run> | C124 | Inspect one Run by full id or unambiguous displayed prefix | N069; P:506 | rename → lf monitor show ID |
| A473 | lf runs --parent | C124 | List every direct child of one Run, without the recent-history cap | N053; P:509 | keep → --parent |
| A474 | lf runs --events | C124 | Print the Run's append-only event stream verbatim | N070; P:516 | rename → lf monitor show ID --events |
| A475 | lf runs --final | C124 | Print the Run's last durable provider conclusion | N071; P:523 | rename → lf monitor show ID --final |
| A476 | lf runs --resume | C124 | Resume the Run's provider-native interactive session | N072; P:530 | merge into → lf session open ID |
| A477 | lf runs --task | C124 | Drill to one roadmap Task by its Linear issue identifier (e.g. W2-122) | N058; P:533 | keep → --task |
| A478 | lf runs --project | C124 | Drill to one roadmap Project by slug | N058; P:536 | keep → --project |
| A479 | lf runs --wave | C124 | Scope to one Wave by name | N058; P:539 | keep → --wave |
| A480 | lf runs --json | C124 | Emit structured evidence for this operation | N060; P:542 | keep → --json |
| A481 | lf runs --help / -h | C124 | Explain this canonical command without performing it | N059; P:497 | keep → --help |
| A482 | lf replay <run> | C125 | Full Run id or an unambiguous displayed prefix | N053; P:547 | keep → <run> |
| A483 | lf replay --help / -h | C125 | Explain this canonical command without performing it | N059; P:545 | keep → --help |
| A484 | lf reply <wave> | C126 | Wave name | N058; P:553 | keep → <wave> |
| A485 | lf reply <text> | C126 | Recent message text (reads stdin when omitted) | N053; P:556 | keep → <text> |
| A486 | lf reply --agent | C126 | Override the provider (e.g. `claude`, `codex`); default is configured | N053; P:559 | keep → --agent |
| A487 | lf reply --max-turns | C126 | Cap provider turns for the reply | N053; P:562 | keep → --max-turns |
| A488 | lf reply --help / -h | C126 | Explain this canonical command without performing it | N059; P:551 | keep → --help |
| A489 | lf chat <text> | C127 | Message text (reads stdin when omitted unless --follow or --history) | N053; P:568 | keep → <text> |
| A490 | lf chat --follow | C127 | Replay and follow the thread while typed lines post into it | N053; P:571 | keep → --follow |
| A491 | lf chat --history | C127 | Read the latest durable turns without requiring a live listener | N053; P:574 | keep → --history |
| A492 | lf chat --json | C127 | Emit structured evidence for this operation | N060; P:577 | keep → --json |
| A493 | lf chat --limit | C127 | Maximum durable turns to return (default: 12) | N053; P:580 | keep → --limit |
| A494 | lf chat --epoch | C127 | Select one immutable conversation epoch | N053; P:583 | keep → --epoch |
| A495 | lf chat --wave / -w | C127 | Target wave by name | N058; P:106 | keep → --wave |
| A496 | lf chat --parent | C127 | Target the invoking wave's parent (escalation up the wave tree) | N053; P:509 | keep → --parent |
| A497 | lf chat --help / -h | C127 | Explain this canonical command without performing it | N059; P:565 | keep → --help |
| A498 | lf op <removed> | C128 | Select removed | N073; P:600 | delete → — |
| A499 | lf op <rest> | C128 | Select rest | N073; P:602 | delete → — |
| A500 | lf op --help / -h | C128 | Explain this canonical command without performing it | N073; P:598 | delete → — |
| A501 | lf ssh --account | C129 | Prefer this origin account when the remote lf chooses a provider | N053; P:610 | keep → --account |
| A502 | lf ssh --only-account | C129 | Restrict remote provider launches to these origin accounts | N053; P:610 | keep → --only-account |
| A503 | lf ssh <target> | C129 | HomeId (preferred), SSH alias, or user@host | N053; P:628 | keep → <target> |
| A504 | lf ssh --repo | C129 | Repository path on the remote, relative to $HOME | N053; P:631 | keep → --repo |
| A505 | lf ssh --secret | C129 | Doppler secret to resolve locally and forward as an env var (repeatable). The Doppler token itself is never forwarded | N053; P:635 | keep → --secret |
| A506 | lf ssh --forward-agent | C129 | Forward the ssh-agent (`ssh -A`). Off by default: git pushes use the forwarded GH_TOKEN over HTTPS, so agent forwarding is unneeded risk | N053; P:639 | keep → --forward-agent |
| A507 | lf ssh <lf_args> | C129 | Arguments for the remote lf. The target is the boundary: every argument after it belongs to the remote invocation | N053; P:643 | keep → <lf_args> |
| A508 | lf ssh --help / -h | C129 | Explain this canonical command without performing it | N059; P:610 | keep → --help |
| A509 | lf run <name> | C130 | Select name | N053; P:647 | keep → <name> |
| A510 | lf run <args> | C130 | Select args | N053; P:649 | keep → <args> |
| A511 | lf run --help / -h | C130 | Explain this canonical command without performing it | N059; P:646 | keep → --help |
| A512 | lf flow --help / -h | C131 | Explain this canonical command without performing it | N059; P:652 | keep → --help |
| A513 | lf flow list --json | C132 | Emit structured evidence for this operation | N060; P:685 | keep → --json |
| A514 | lf flow list --help / -h | C132 | Explain this canonical command without performing it | N059; P:683 | keep → --help |
| A515 | lf flow show <name> | C133 | Select name | N053; P:688 | keep → <name> |
| A516 | lf flow show --help / -h | C133 | Explain this canonical command without performing it | N059; P:688 | keep → --help |
| A517 | lf flow validate <name> | C134 | Select name | N053; P:690 | keep → <name> |
| A518 | lf flow validate --help / -h | C134 | Explain this canonical command without performing it | N059; P:690 | keep → --help |
| A519 | lf flow decide <decision> | C135 | Select decision | N053; P:694 | keep → <decision> |
| A520 | lf flow decide <summary> | C135 | Select summary | N053; P:696 | keep → <summary> |
| A521 | lf flow decide --help / -h | C135 | Explain this canonical command without performing it | N059; P:692 | keep → --help |
| A522 | lf flow route <path> | C136 | Select path | N053; P:699 | keep → <path> |
| A523 | lf flow route --help / -h | C136 | Explain this canonical command without performing it | N059; P:699 | keep → --help |
| A524 | lf flow blocked <reason> | C137 | Select reason | N053; P:703 | keep → <reason> |
| A525 | lf flow blocked --help / -h | C137 | Explain this canonical command without performing it | N059; P:701 | keep → --help |
| A526 | lf flow resume <invocation> | C138 | Select invocation | N053; P:707 | keep → <invocation> |
| A527 | lf flow resume --retry | C138 | Select retry | N053; P:709 | keep → --retry |
| A528 | lf flow resume --help / -h | C138 | Explain this canonical command without performing it | N059; P:706 | keep → --help |
| A529 | lf skill --help / -h | C139 | Explain this canonical command without performing it | N059; P:657 | keep → --help |
| A530 | lf skill list <namespace> | C140 | Select namespace | N053; P:670 | keep → <namespace> |
| A531 | lf skill list --json | C140 | Emit structured evidence for this operation | N060; P:672 | keep → --json |
| A532 | lf skill list --help / -h | C140 | Explain this canonical command without performing it | N059; P:669 | keep → --help |
| A533 | lf skill show <name> | C141 | Select name | N074; P:675 | merge into → lf help skill <name> |
| A534 | lf skill show --help / -h | C141 | Explain this canonical command without performing it | N074; P:675 | merge into → lf help skill --help |

## Rationale key

- **N000**: Bare launch versus help: keep existing launch endpoint; startup help must describe it.
- **N001**: Distinct operation described in purpose; no equivalent identified.
- **N002**: Creates a review Session; session open continues an existing conversation.
- **N003**: Conversation continuation; retire runs --resume into this owner.
- **N004**: Author reports readiness; complete requires review completion authority.
- **N005**: LOO-298 owns exact process evidence and Exec identity; migrate its child caller together.
- **N006**: Published machine installation; rebase refreshes a source checkout.
- **N007**: CI detail and watch differ from single PR summary.
- **N008**: Rotates settled PR chain; --next on delivery records future continuation intent.
- **N009**: Ready PR publication differs from draft/open and from merge intent.
- **N010**: Publishes a draft and opens browser; publish produces ready review. Demo choice R08.
- **N011**: Prepares review without merge intent; arm/land request merge.
- **N012**: Requests merge and returns; land watches and repairs through settlement.
- **N013**: Waits for merge settlement; task complete records disposition separately.
- **N014**: Discards branch/PR artifacts; task delete removes provider planning object.
- **N015**: Untracked checkout allocation; task checkout retains tracked ownership.
- **N016**: Physical checkouts differ from Task portfolio.
- **N017**: Cached credentials/capacity; --verify explicitly refreshes; bare account adds same-reader overview.
- **N018**: Authenticate/register a login; set edits metadata without authenticating.
- **N019**: Account properties/browser venues; route set orders spending accounts.
- **N020**: Whole release workflow; individual steps remain recovery/CI primitives.
- **N021**: Release assets/publication, not Task PR publication.
- **N022**: Machine identity differs from participant name and provider account.
- **N023**: Route evidence does not move Wave placement or establish remote liveness.
- **N024**: Reconciles declared jobs; add/remove author manual entries; install schedule updates software.
- **N025**: Merge authored local goals and registry evidence; connection is optional.
- **N026**: Allocation only, no worker launch; distinct from task run.
- **N027**: Continues saved workflow; restart replaces it; direct run starts an invocation.
- **N028**: Changed-file inventory; diff is patch content, file is full content.
- **N029**: Task disposition; Session complete returns feedback and cannot substitute for it.
- **N030**: Provider deletion with preserved local execution evidence; not interruption.
- **N031**: Planning title/notes; comment appends direction without rewriting description.
- **N032**: Thread read/append; preserve read-only omission and explicit write input.
- **N033**: Measures tracked repository files/history; repo owns it, not Home (tokens.rs:46). Distinct from provider usage.
- **N034**: Measured provider consumption; account status reports capacity, not spend totals.
- **N035**: OS process/call-tree sample; active is provider discovery, not equivalent liveness.
- **N036**: Continuous terminal presentation of ps, one sample when redirected; retained explicit request.
- **N037**: Exact owned process cleanup; worktree prune deletes eligible checkout directories.
- **N038**: Diagnostics; monitor displays work, doctor investigates consistency.
- **N039**: Catalog lists executable names; object list commands enumerate domain records.
- **N040**: Explains invocations without execution; separate from object state.
- **N041**: Existing portfolio becomes task list; add unlinked checkout/PR facts without Task creation.
- **N042**: Ordered cross-Work facts; history list is individual execution records.
- **N043**: Split multiplexed history/inspect/active/resume into monitor list/show/active and session open after LOO-298.
- **N044**: Starts another execution, not passive playback; disclose effects. Demo choice R08.
- **N045**: Retired namespace rejection shim; no operation remains.
- **N046**: Named execution prefers a Flow; skill/flow explicitly select kind. Not the retired Run object.
- **N047**: Captured workflow execution and cursor controls differ from provider conversation.
- **N048**: Shows captured/expanded topology; typed help includes instructions/metadata.
- **N049**: Cursor verdict; blocked reports obstacle, route selects captured XOR branch.
- **N050**: Captured invocation continuation; task run also owns Task worker admission.
- **N051**: Explicit kind avoids command and Flow collisions; keep reserved-name escape.
- **N052**: Same definition_help renderer as typed help, but reserved-name escape currently fails. Implement help skill -- NAME before deleting show.
- **N053**: Input to this operation; same spelling elsewhere selects that other object.
- **N054**: Preference can fall back; restriction limits descendant authority. Preserve distinct semantics (R04).
- **N055**: Different authority/override scopes; do not merge confirmation, force and provider permission (R07).
- **N056**: Positive launch mode overrides configured default; retain both modes, delete uppercase aliases.
- **N057**: Positive/negative overrides differ when configuration sets a default; not inert synonyms.
- **N058**: Scope of this read/write, distinct from root launch attribution; resolve explicit target before effects (R04).
- **N059**: Shared Clap help affordance, not another domain operation.
- **N060**: Keep DTO contract; errors/progress on stderr, streams explicit (R03).
- **N061**: Installation transaction artifact/target identity; required by promotion/recovery, not a public tuning knob.
- **N062**: Preview existing operation, no second writer. Plan/preview vocabulary requires demo decision R07.
- **N063**: Only json is recognized; other values silently select text (ops/mod.rs:1655).
- **N064**: Ignored: run_wt destructures List with ..; no effect (ops/mod.rs:1516).
- **N065**: Commit plus push/draft publication duplicates the explicit PR operation; migrate composition.
- **N066**: Ignored in bin/lf.rs:1680; cached reads are the default. Migrate builtin callers.
- **N067**: Provider discovery owns its own operation; do not infer from unfinished history.
- **N068**: Continuous discovery is an explicit stream, independent of history listing.
- **N069**: Single-object lookup moves out of list; ID is Exec or Session after LOO-298.
- **N070**: Event evidence for one Exec/Session; LOO-298 reader contract required.
- **N071**: Durable provider conclusion belongs to the selected Session/Exec.
- **N072**: Provider conversation belongs to Session; use stable Session identity after LOO-298.
- **N073**: Parent is a retired error-only namespace.
- **N074**: Use the shared typed-help operation.

## Caller evidence

[E000]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L452
[E001]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L72
[E002]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L185
[E003]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/config.md#L4
[E004]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L128
[E005]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/webhook.rs#L133
[E006]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/getting-started.md#L22
[E007]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/index.md#L73
[E008]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L294
[E009]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/LOOPFLOW.md#L85
[E010]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/pr-review.md#L67
[E011]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L199
[E012]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/screenshot.rs#L29
[E013]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture/codebase.md#L67
[E014]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/util.rs#L787
[E015]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/LOOPFLOW.md#L74
[E016]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L40
[E017]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/rebase-conflicts.md#L55
[E018]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/README.md#L115
[E019]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/task/mod.rs#L796
[E020]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/util.rs#L594
[E021]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L259
[E022]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/SessionFixture.swift#L54
[E023]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L16
[E024]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L269
[E025]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/SessionFixture.swift#L57
[E026]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L291
[E027]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/SessionFixture.swift#L61
[E028]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/flow_session.rs#L270
[E029]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L282
[E030]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/surfaces/human-present.md#L9
[E031]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/flow_session.rs#L234
[E032]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/human_session.rs#L950
[E033]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/human_session.rs#L2077
[E034]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/human_session.rs#L2093
[E035]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/human_session.rs#L913
[E036]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/install.py#L416
[E037]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L1444
[E038]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install.rs#L1
[E039]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install/published.rs#L221
[E040]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L197
[E041]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/global_commands.rs#L266
[E042]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install.rs#L2957
[E043]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/publish_release.py#L164
[E044]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install.rs#L718
[E045]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install/published.rs#L99
[E046]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install/recovery.rs#L126
[E047]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install.rs#L1453
[E048]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/global_commands.rs#L108
[E049]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install.rs#L3058
[E050]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/install.py#L508
[E051]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/store/mod.rs#L337
[E052]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/store/sqlite.rs#L352
[E053]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L70
[E054]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/task/mod.rs#L1120
[E055]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ops/mod.rs#L143
[E056]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ops/mod.rs#L1986
[E057]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wt_ci_logs_tests.rs#L144
[E058]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowTests/WaveChatConnectionTests.swift#L461
[E059]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/task.rs#L3276
[E060]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L73
[E061]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/LOOPFLOW.md#L24
[E062]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/pr-publish.md#L12
[E063]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/land.rs#L194
[E064]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/LOOPFLOW.md#L25
[E065]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/pr-land.md#L31
[E066]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/pr-submit.md#L11
[E067]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pr_landing.rs#L327
[E068]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/LOOPFLOW.md#L26
[E069]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/pr-land.md#L30
[E070]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/run.rs#L719
[E071]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/waves.rs#L1550
[E072]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pr_landing.rs#L237
[E073]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/task/skill/ship-decomposed.md#L76
[E074]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/task.rs#L2912
[E075]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/agent.rs#L1742
[E076]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/output.rs#L108
[E077]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/telemetry.rs#L2
[E078]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_checkout_refresh.py#L134
[E079]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/worktree_tests.rs#L392
[E080]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/wave/skill/review-open-work.md#L48
[E081]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/troubleshooting.md#L107
[E082]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/troubleshooting.md#L108
[E083]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ops/mod.rs#L1967
[E084]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/troubleshooting.md#L114
[E085]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/release_tests.rs#L1468
[E086]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L1627
[E087]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ops/mod.rs#L228
[E088]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/checkout.rs#L182
[E089]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/LOOPFLOW.md#L21
[E090]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/start-chapter.md#L77
[E091]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/task/skill/ci-fix.md#L18
[E092]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L66
[E093]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/agent.rs#L2156
[E094]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/auth_status.rs#L350
[E095]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/demo_profile_routing.py#L151
[E096]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/publish_release.py#L124
[E097]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/subscriptions.md#L34
[E098]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pm/oauth_tests.rs#L236
[E099]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/demo_profile_routing.py#L107
[E100]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/subscriptions.md#L89
[E101]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/auth.rs#L972
[E102]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/demo_profile_routing.py#L132
[E103]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/profile.rs#L24
[E104]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L38
[E105]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/config.md#L439
[E106]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/demo_profile_routing.py#L154
[E107]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/subscriptions.md#L17
[E108]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/canonicalize_migrations.py#L8
[E109]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/check_migrations.py#L16
[E110]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/install.py#L9
[E111]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/new_migration.py#L14
[E112]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/release.rs#L733
[E113]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/store/MIGRATIONS.md#L98
[E114]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/config.md#L187
[E115]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/publish_release.py#L193
[E116]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_release_publisher.py#L178
[E117]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/wave/relocate.rs#L265
[E118]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pm.rs#L328
[E119]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L193
[E120]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L693
[E121]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L880
[E122]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ops/mod.rs#L825
[E123]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L881
[E124]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L25
[E125]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/home.rs#L11
[E126]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L39
[E127]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/verify_branch_data.py#L69
[E128]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/home.rs#L28
[E129]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/verify_branch_data.py#L90
[E130]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/home.rs#L54
[E131]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L235
[E132]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L186
[E133]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L47
[E134]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/doctor.rs#L442
[E135]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/cron.rs#L433
[E136]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L479
[E137]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L81
[E138]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_release_automation.py#L220
[E139]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L90
[E140]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_release_automation.py#L218
[E141]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L101
[E142]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L78
[E143]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_release_automation.py#L219
[E144]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L103
[E145]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L128
[E146]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/cron.rs#L593
[E147]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L116
[E148]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_release_automation.py#L221
[E149]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L105
[E150]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L106
[E151]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Models/Wave.swift#L7
[E152]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Models/WaveViewModel.swift#L30
[E153]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Models/WaveWorkMap.swift#L92
[E154]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L7
[E155]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/AppTestMode.swift#L109
[E156]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L82
[E157]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/Views/RoadmapView.swift#L521
[E158]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/home.rs#L74
[E159]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pm.rs#L286
[E160]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L171
[E161]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/wave/skill/split-wave.md#L53
[E162]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L302
[E163]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ssh.rs#L15
[E164]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/waves.rs#L728
[E165]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L99
[E166]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L589
[E167]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/status_tests.rs#L538
[E168]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L239
[E169]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L150
[E170]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L651
[E171]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/wave/README.md#L106
[E172]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L188
[E173]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture/homes.md#L133
[E174]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L588
[E175]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/wave/README.md#L59
[E176]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/wave/recovery.rs#L105
[E177]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L502
[E178]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/waves.rs#L900
[E179]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/chapter.rs#L289
[E180]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/chapter_tests.rs#L599
[E181]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L74
[E182]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L189
[E183]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L530
[E184]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L72
[E185]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/task/skill/prompt.md#L157
[E186]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/wave/skill/split-wave.md#L54
[E187]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L93
[E188]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L2239
[E189]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/wave/README.md#L13
[E190]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/wave/mod.rs#L21
[E191]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/wave/resident.rs#L3
[E192]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Models/TaskComments.swift#L3
[E193]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L32
[E194]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/README.md#L61
[E195]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L187
[E196]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture/codebase.md#L65
[E197]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L100
[E198]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L114
[E199]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/task/skill/advance.md#L42
[E200]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L188
[E201]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/task/mod.rs#L854
[E202]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L129
[E203]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pm/task_planning_tests.rs#L352
[E204]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/task_pm.rs#L154
[E205]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L756
[E206]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/waves.rs#L1039
[E207]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/work/task/mod.rs#L130
[E208]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L176
[E209]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L374
[E210]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/task.rs#L1976
[E211]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L228
[E212]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L375
[E213]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowTests/RegistryQueryTests.swift#L561
[E214]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L247
[E215]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L376
[E216]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L253
[E217]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L377
[E218]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L361
[E219]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/waves.md#L404
[E220]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pm.rs#L1391
[E221]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pm/task_planning_tests.rs#L571
[E222]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L95
[E223]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/agent-api.md#L130
[E224]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L216
[E225]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L341
[E226]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L82
[E227]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L202
[E228]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/README.md#L106
[E229]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L135
[E230]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/agent-api.md#L103
[E231]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/conducting.md#L125
[E232]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/agent-api.md#L39
[E233]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/getting-started.md#L99
[E234]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L348
[E235]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L196
[E236]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/task_flow.rs#L76
[E237]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L172
[E238]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/tokens.rs#L46
[E239]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L325
[E240]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L265
[E241]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/Views/TelemetryDashboardView.swift#L422
[E242]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/usage.rs#L1
[E243]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/subscription.rs#L8
[E244]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/agent-api.md#L161
[E245]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L1783
[E246]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_architecture.py#L165
[E247]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/flow.rs#L446
[E248]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ci.rs#L1
[E249]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L192
[E250]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/test.py#L521
[E251]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/top.rs#L1
[E252]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L182
[E253]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L181
[E254]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L200
[E255]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install.rs#L576
[E256]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L184
[E257]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L191
[E258]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/Views/TelemetryDashboardView.swift#L566
[E259]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/doctor.rs#L1
[E260]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/.github/workflows/package-build.yml#L67
[E261]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/error.rs#L17
[E262]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/navigation.rs#L268
[E263]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L14
[E264]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Models/NowProjection.swift#L14
[E265]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/Views/RoadmapView.swift#L45
[E266]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Models/WorkActivity.swift#L3
[E267]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/activity.rs#L1
[E268]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/store/sqlite/durable.rs#L501
[E269]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L26
[E270]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/Views/TaskRunsView.swift#L6
[E271]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L198
[E272]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture/execution.md#L172
[E273]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/conducting.md#L75
[E274]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/wave/README.md#L6
[E275]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/reply.rs#L1
[E276]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/WaveChatClient.swift#L263
[E277]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/README.md#L266
[E278]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/wave/README.md#L4
[E279]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L771
[E280]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L2301
[E281]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L180
[E282]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/home.rs#L184
[E283]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ssh.rs#L1
[E284]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/authoring.md#L103
[E285]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/task/mod.rs#L749
[E286]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/flow.rs#L946
[E287]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/flow.rs#L44
[E288]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L182
[E289]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L36
[E290]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L192
[E291]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/global_commands.rs#L130
[E292]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L193
[E293]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/global_commands.rs#L131
[E294]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/flow.rs#L426
[E295]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L475
[E296]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L462
[E297]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture/planning.md#L96
[E298]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/flow.rs#L177
[E299]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L476
[E300]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/flow.rs#L93
[E301]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/flow_session.rs#L255
[E302]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L489
[E303]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/error.rs#L19
[E304]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/list.rs#L79
[E305]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L34
[E306]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L168
[E307]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/cli_discovery.rs#L106

## Extra aliases

| Canonical row | Extra spelling | Verdict |
|---|---|---|
| A001 `lf --clipboard` | `-C` | delete; retain primary short/long spellings |
| A002 `lf --model` | `-M` | delete; retain primary short/long spellings |
| A007 `lf --interactive` | `-I` | delete; retain primary short/long spellings |
| A008 `lf --batch` | `-B` | delete; retain primary short/long spellings |
| A018 `lf --wave` | `-W` | delete; retain primary short/long spellings |
| A111 `lf pr publish --model` | `-M` | delete; retain primary short/long spellings |
| A115 `lf pr open --model` | `-M` | delete; retain primary short/long spellings |
| A171 `lf commit --message` | `-M` | delete; retain primary short/long spellings |
| A172 `lf commit --push` | `-P` | delete; retain primary short/long spellings |
| C041 `lf wt remove` | `rm` | delete; canonical operation only |

## Dynamic/default entry points

| Surface | Owner and purpose | Caller/source | Overlap | Verdict |
|---|---|---|---|---|
| Bare `lf` | Existing default agent launch | `bin/lf.rs::run_default_agent` | Root help must explain default; not a new setup operation | keep; demo choice if switching bare default to help |
| `lf NAME` | Command-first, then flow-first definition lookup | `bin/lf.rs`, `lf/navigation.rs` | Derived convenience, not an alias registry | keep |
| `lf skill NAME` | Explicit skill selection | `ops/human_session.rs` and `ops/cron.rs` | Required to avoid same-named Flow and command collisions | keep |
| `lf flow NAME` | Explicit Flow selection | `lf/commands/flow.rs` | Saved invocation and cursor differ from skill launch | keep |

## Required additions and cross-cutting work

- `monitor` overview and `account` overview reuse existing evidence readers; include missingness and next action. Neither creates a second scheduler or credential store.
- `monitor show ID` and `monitor active` replace the multiplexed `runs` path using LOO-298 Exec/Session owners. History filters, parent identity and DTOs follow that model.
- `task list` adds unlinked checkout/PR evidence; `wave list` includes locally authored goals without requiring planning credentials. These carry earlier omitted recommendations forward.
- First local result uses existing direct execution in a fresh directory before planning connection. Verify a real provider result; fix actual setup obstacles rather than introduce synthetic Tasks.
- Foreground/background/remote launch readiness checks required access at the destination while retaining inherited restrictions.
- Every rename/delete migrates parser, typed Flow dispatch, skill text, scripts, Desktop argv and tests in the same cut. Saved execution identity remains LOO-298's responsibility.
- Add no aliases beyond the explicitly selected monitor/mon exception. Account has no short name.
- Retain draft/open behavior pending Jack's demo choice; do not silently reinterpret mutation as a view.
- Public docs lose draft/deferral labels only after the above paths are implemented and the public-CLI walkthrough passes. No landing before Jack's demo review.

## Review findings

- `wt list --full` is parsed but ignored. `wave status --no-sync` is parsed and discarded. Both delete verdicts are based on handlers, not missing caller strings.
- `wt list --format` accepts arbitrary strings but only recognizes `json`; replace with `--json` and retain the same serialized data.
- `skill show` calls the same definition-help renderer as `help skill NAME`; merge the duplicate operation into typed help, preserving reserved-name inspection.
- `commit --push` duplicates draft publication owned by PR open. Preserve commit-only behavior and compose PR publication explicitly.
- Do not delete required internal installation/process callbacks merely because public help hides them. Exact artifact, worker, Session and process evidence grant different authority.
- Counts include namespace and help entries. Moving nodes does not itself reduce leaf operations; report real deleted arguments/aliases separately at demo.
