# LOO-620 assumptions

- **Receipt shape unchanged.** `--legacy-app-target` stays single-valued: the switch
  receipt (`ActivationTargets.legacy_app`) is read by released and branch lf across a
  switch. Earlier `~/Applications` release installs are found by
  `install::earlier_release_installs` (bundle-id checked) instead of more flags.
- **No sweep of old sibling orphans.** Leftover `/Applications/.Loopflow.app.promote.*`
  / `.superseded.*` from earlier lf aren't removed; Jack's `/Applications` held only
  `Loopflow.app` on 2026-11-02. Add a one-line sibling sweep if another Machine has them.
- **No full Launch Services purge.** Other on-disk copies already registered with
  `com.loopflow.mac` (old benchmark/dist builds) stay registered; `lf open` no longer
  reaches them, and new builds can't take the id. Removing them would mean parsing
  `lsregister -dump`.
- **Dev builds share `com.loopflow.mac.dev`.** local-bin, Xcode and `Loopflow Dev`
  have the same id, so `LF_DESKTOP=com.loopflow.mac.dev` can reach any of them. They're
  all dev apps; a separate local-bin id is a small follow-up if it matters.
