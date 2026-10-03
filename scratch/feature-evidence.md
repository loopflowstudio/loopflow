# Feature inventory and historical evidence

2026-10-01. Research supporting Jack Heart's feature exploration. Loopflow checkout: `3d1f76780`; studio checkout: `7fa88ca`. Source and history inspection, not a live acceptance run. Historical copy records proposals and positioning, not proof that behavior still exists.

## The six requested subsystems

| Subsystem | Loopflow's approach | What another part can rely on | Current evidence |
|---|---|---|---|
| Worktrees / Git | One Task retains its checkout and PR history; Git remains authoritative for commits | Reviews, retries, and delivery return to the associated work | `docs/architecture/delivery.md`; `rust/loopflow/src/work/task/`; `rust/loopflow/src/ops/task.rs` |
| CI / PR review | Configurable review stops and revision loops; recorded PR landing; watcher starts eligible repairs | A failed required check can be routed into repair of the same Task delivery | `engine/builtins/task/flow/{feature,task-design,pursue,queue}.yaml` under `rust/loopflow/src/`; `ops/ci_watch.rs`; `ops/pr_landing.rs` |
| Project management | Linear Projects own plans; Tasks connect planning to checkout work; Waves retain purpose across plans | Automation and the UI use the same associations between plan and execution | `docs/waves.md`; `docs/architecture/planning.md`; `rust/loopflow/src/store/sqlite/task_work.rs` |
| Credential forwarding | Foreground SSH borrows account access through an origin broker; durable remote work needs remote credentials | Remote agent launch can select an account without installing its saved login there | `docs/security.md`; `docs/subscriptions.md`; `rust/loopflow/src/lf/commands/ssh.rs` |
| Skills / workflows | Markdown instructions plus captured YAML graphs containing commands, review boundaries, branches, and loops | Later steps receive prior artifacts; interrupted execution has a saved position | `docs/authoring.md`; `rust/loopflow/src/engine/flow.rs`; `rust/loopflow/src/ops/flow.rs` |
| Memory / context | Reviewed files hold durable lessons; scratch holds current work; relevant Wave ancestry and budgets shape input | New work can receive lessons and goals without manually rebuilding its context | `docs/waves.md`; `rust/loopflow/src/work/wave/context.rs`; `rust/loopflow/src/engine/context_budget.rs` |

CI limitation checked in code: `ops/ci_watch.rs::unrepaired()` distinguishes “not armed” and “no Task”; watching arbitrary red PRs does not authorize repairing all of them. `task_automation.rs` checks pending review and other work before admission. Review completion supplies feedback; the following decision controls the Flow's edge.

Credential distinction: Claude/Codex accounts use a lazy subscription broker. GitHub, Linear, and OpenCode Zen use one effective forwarded credential in the process environment. “Every credential is a short-lived brokered token” would be false. The remote OS user can access the authority it receives; no security sandbox is implied.

## Additional recognizable capabilities

These extend the inventory without expanding the first homepage section.

| Capability | Plain-language explanation | Evidence |
|---|---|---|
| Durable conversations | Find, name, and return to a conversation after its original command stops | `docs/conducting.md`; `rust/loopflow/src/store/sessions.rs`; commit `d296b4805` |
| Saved workflow progress | Continue the captured workflow from its recorded position | `docs/authoring.md`; `rust/loopflow/src/store/flows.rs` |
| Provider choice | Use an installed Claude Code, Codex, or OpenCode agent | `docs/getting-started.md`; `docs/config.md` |
| Multiple accounts / routing | Choose accounts per repository and inspect their observed capacity | `docs/subscriptions.md` |
| Mac workspace and CLI | Navigate Tasks, conversations, files, and workflow progress through two surfaces over the same records | `docs/conducting.md`; `swift/LoopflowMac/Views/TaskWorkspaceView.swift` |
| Attention and usage | See active work, waiting decisions, history, and recorded provider usage | `docs/conducting.md` |
| Agent API | Let another agent inspect and operate work through the same commands and JSON observations | `docs/agent-api.md` |
| Scheduled continuation | Periodically check enrolled Tasks and deliveries and continue eligible work | `rust/loopflow/src/ops/task_automation.rs`; commit `93395e2c0` |
| Nested goals | Bring parent and child goals and memory into the appropriate work | `docs/waves.md`; commit `963bf8e0a` |
| Plan rotation | Change the current plan while preserving started work and its identity | `docs/waves.md`; `docs/architecture/planning.md` |
| Reusable engineering practice | Builtin skills cover design, debugging, simplification, QA, testing audits, and delivery | `rust/loopflow/src/engine/builtins/` |
| Install / release operations | Install published builds, schedule updates, and run configured release workflows | `README.md`; `release/README.md` |

The most recent CI watcher, scheduled reconciliation, nested Waves, and ongoing repository/Wave conversations landed after the `v0.12.29` release commit. Check the actual published release before advertising them as downloadable features. `RELEASE_NOTES.md` also explicitly leaves provider/Desktop/Linear acceptance and installed conversion evidence open. Do not turn source inspection into a claim of measured reliability.

## What the history contributes

1. **2025-12-08, Loopflow `35757f4ee`:** the initial README describes launching Claude with repository instructions, a reusable perspective, a task prompt, and a commit wrapper. Composition of familiar tools is present from the start; today's system greatly extends the continuity around it.
2. **2026-01-20, studio `08d3433`:** the site teaches Step → Flow → Loop. This makes progression understandable, but leads with new vocabulary; the autonomous loop is explicitly marked coming soon.
3. **2026-01-21, studio `9043c47`:** variants pitch scale, craft, and flow. The sampled feedback/quote files are empty templates, so this commit supplies positioning hypotheses, not customer validation.
4. **2026-02-27, studio `d2327c0`:** “Welcome to wave coding” and “Waves that remember, adapt, and ship” lead. Its design explicitly calls for restoring concrete specifics: prompts as files, context control, provider choice, shipping, review, and overnight work. Abstract identity versus recognizable functionality was already a tension.
5. **2026-02-27, studio `26c0c2f`:** removes variant machinery for one demo-oriented page and six explicit capabilities: persistent context, diffable prompts, composable execution, stimulus-driven automation, shipping control, and shared surfaces. The video is a placeholder at this point. Its obsolete execution vocabulary must not be revived as current behavior.
6. **2026-06-30, Loopflow `6f8846ba6`:** imports the public website. Studio removes its copy on 2026-07-03 (`f9d53f2`). The current site is owned here.
7. **2026-07-17, Loopflow `547708b3f`:** retells the docs/site agent-first. Current `website/content.yaml` emphasizes plan/build/result, problems, and editable files; navigation offers Docs, GitHub, and Install. Many substantial capabilities now require reading the docs to discover.
8. **September–October, current Loopflow:** durable conversation and Flow records, continued Task identity, shared work associations, context budgets, scheduling, and CI repair add concrete connections beneath the feature list. These are stronger evidence for the integrated-system story than a claim to have invented the individual features.

Release notes for `v0.10.0` and `v0.12.0` help explain evolution, but also describe removed authorities and interfaces: Asana, resident Wave services, old daemon/attach paths, and older ownership models. Current docs and code take precedence.

## Editorial findings

Lead with subsystem names people already recognize; explain the approach before introducing domain vocabulary. Each entry should answer “what does it do for me?” and “how does Loopflow handle it?” The final example can carry the integration argument without adding a second abstract paragraph to every entry.

Avoid “all tools anyone could need” or competitive exclusivity. Jack's thesis can be demonstrated through specific handoffs. The existing “work in progress never collides” copy overstates worktree isolation; a replacement should explain separate working copies and integration explicitly. The existing universal demo-before-done statement also needs qualification because workflows are customizable.

No external market research was performed. Familiarity of these patterns is a positioning hypothesis, not a measured claim about adoption or competitors.

## Individual instrument and company practice

Jack Heart explicitly connected workflow freedom to model neutrality and freedom from capture inside a vendor's system. Jack also identified a company-level benefit: a shared language and toolset for exploring and defining agent practice. These are product direction supplied in this conversation, not conclusions inferred from historical commits.

Concrete support: `LICENSE` permits use, modification, and redistribution under MIT; `docs/getting-started.md` lists multiple installed agent providers; `docs/subscriptions.md` describes independently managed accounts; `docs/authoring.md` specifies repo-owned skill and workflow files; `docs/waves.md` describes reviewed memory files. Together these make working methods inspectable and changeable. Git review of those files supplies a practical way to share and improve methods without inventing a separate company governance service.

Keep the distinction between authored method and recorded execution: editable workflow files are easy to inspect and share; captured progress and conversation history have their own local records. Do not say every part of the system is a plain text file or that a workflow can execute unchanged under any other tool.
