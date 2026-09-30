# Decisions and coordination

- Jack Heart selected `lf account`, without a short alias, on 2026-09-30:
  “i thought we decided on lf account instead of lf identity”. `home id`
  retains machine identity. Growth memory records this correction.
- Jack Heart expanded scope to every command/subcommand/option, including
  hidden/internal ones. Catalog and commit precede implementation. The initial
  delivery-owner edits were restored before collecting the catalog.
- LOO-298 owns Exec/AgentSession/FlowSession. Local history provides naming
  evidence, but that model is absent from this branch's base. Monitor must
  integrate its owners before replacing Run inspection; no relabeling shim.

- Jack Heart's September 30 steers supersede rename-only implementation: cull
  concepts against LOO-298/334/339/340 first; guides use shortest unique commands;
  keep wt, top and ps; mon is a unique prefix, not an alias. The catalog's
  destination audit records each boundary and leaves runs view cuts open at demo.
- LOO-334 scratch still discusses a private local planning store; Jack's newer
  direction says Linear/Git establish Tasks and the DB records execution. Follow
  the newer instruction; reconcile integration without inventing Task records.
- LOO-329/330 sibling drafts disagree on rename-preserved Wave identity. Retain
  the rename operation's concept but do not decide their identity contract here.
- LOO-340 had no checkout in the first roadmap read. A later read found
  loopflow.keep-account-status-live-and; its plan now implements live status,
  replaces --verify with --cached, and explicitly forbids per-step overrides.
  One per-provider selection is captured for the whole launch and its children.

- 2026-09-30 integration: use `lf rebase --manual` onto committed LOO-340
  `32b6eef00` for its independent live-status/browser slices. Preserve this
  branch's Account spelling and LOO-339 identity checks. No PR base change,
  sibling edit, installed-store conversion or account repair is authorized.
  LOO-298/334 remain unfinished; their model is not replaced or relabeled here.
  LOO-340's remaining store/bundle/reset work still requires later integration.

- 2026-09-30 latest steer supersedes the demo/review wait: Jack Heart delegates
  remaining cull decisions; the supervisor ships after matching CLI/catalog and
  focused proof. Monitoring keep-or-cut choices are now selected in the catalog.
  The main rebase is complete at `7164a0d81` over #1360 (`3dc89bc9a`).
- Current dependency evidence supersedes the older unavailable-branch notes:
  LOO-298 `25548d567` implements a current-state cutover and explicitly retires
  older history-import obligations, while retaining current resumable Sessions.
  LOO-334 `1fd99a284` restarts its accepted planning design. Integrating either
  requires reconciling these actual owners with the now-deleted resident service;
  copying command names alone cannot satisfy Monitor or Task discovery.

- 2026-09-30: Jack Heart's newest steer authorizes autonomous catalog decisions
  and removes demo/review with him. The supervising session ships after focused
  proof and catalog agreement. It does not authorize losing managed Task Flow
  state. LOO-298 local integration exposed non-equivalence of ordinary attributed
  Flow capture and managed Task continuation, plus a refused PR-base update.
  Exact evidence and remaining obligations are in the working design.

- 2026-09-30: the subsequent `lf rebase --manual main` completed and reconciled
  Task-base lineage at f0a2a57c6. The earlier refusal is resolved, not a current
  blocker. Managed Task Flow versus independent attributed Flow remains distinct.
