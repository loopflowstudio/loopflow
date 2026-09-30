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
