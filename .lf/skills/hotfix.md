Hotfix the lf installed on this Machine, then carry the fix to main.

A hotfix changes code paths only. It is built on the installed commit, proven on
a clone of the live store, tested, built once, promoted, rolled over, and opened
as a PR to main in the same run.

## Limits

- No schema change. The store's schema must match the migration chain exactly,
  so an added index or column makes every lf, old or new, refuse the store. If
  the fix needs one, stop: it belongs in a release.
- No index of the live store's rows by hand, no `sqlite3` writes to
  `~/.lf/loopflow.db`. Reproduce and measure on the clone.
- `lf release` cuts from main only. Don't use it for a hotfix.
- `~/.lf/bin/hotfix-*` are leftovers from an earlier hand-made hotfix. Ignore them.

## 1. Worktree from the installed commit

```bash
python3 -c 'import json,os; s=json.load(open(os.path.expanduser("~/.lf-machine/install/active.json")))["selection"]["artifact_set"]; print(s["source_revision"], s["source_identity"], *(a["path"] for a in s["artifacts"] if a["role"]["kind"] == "cli"))'
# <source_revision> <source_identity> <active CLI path>
```

`source_revision` is the base: the installed release tag's commit, or the
previous hotfix's commit. Build on it, never on main, so the hotfix carries no
unreleased migration.

```bash
git fetch origin --tags
git worktree add -b hotfix/<version>-<slug> ../loopflow.hotfix-<slug> <source_revision>
```

Make the fix there and commit it. Then confirm it touches no migration:

```bash
git diff --stat <source_revision> -- rust/loopflow/src/store/migrations   # must be empty
```

## 2. Reproduce and measure on a clone

`cp -c` makes an APFS clone: it takes about 0.01 s for a 10 GB store, and
blocks are shared until written. Check headroom first, since the clone grows as
it diverges.

```bash
df -h ~
clone=$(mktemp -d "$HOME/lf-hotfix-clone.XXXXXX")
cp -c ~/.lf/loopflow.db ~/.lf/loopflow.db-wal "$clone"/
sqlite3 "$clone/loopflow.db" 'PRAGMA integrity_check(1);'   # ok
```

Run the slow or failing statement against the clone with `.timer on`, then the
fixed statement, on the same rows. Take real rows from the clone rather than
synthetic ones, and use `EXPLAIN QUERY PLAN` to show why. Example from the
0.14.16 incident (FK-checked delete against an in-place update):

```bash
sqlite3 "$clone/loopflow.db" <<'SQL'
.timer on
PRAGMA foreign_keys=ON;
EXPLAIN QUERY PLAN SELECT 1 FROM session_events WHERE captured_event=?;
BEGIN; DELETE FROM session_events WHERE seq=<observed seq>; ROLLBACK;
BEGIN; UPDATE session_events SET payload=payload WHERE seq=<observed seq>; ROLLBACK;
SQL
```

Record cold and warm times for both. If the fixed path isn't clearly faster or
correct on the clone, stop; the diagnosis is wrong. Remove the clone when done:
`rm -rf "$clone"`.

Never simulate load (`yes`, `stress`) to reproduce a race. One real statement
on a real copy is the measurement.

## 3. Focused tests, before anything is built for install

From the hotfix worktree, with inherited `LF_*` unset and stdin closed:

```bash
env $(env | grep -o '^LF_[A-Z_]*' | sed 's/^/-u /') \
  cargo test -p loopflow <focused filter> < /dev/null
```

Add a test that fails without the fix when one is cheap. A failure stops the run.

## 4. One release build

A plain release build is enough: promote accepts any draft-free build on the
store's exact frontier and refuses one carrying a migration draft.

```bash
cargo build --release -p loopflow --bin lf
./target/release/lf --version   # lf <version>+<rev> (hotfix <rev> on <tag>)
```

## 5. Preview, then promote

Run promote with the binary you just built; it promotes itself.

```bash
./target/release/lf self install promote --cli-target ~/.local/bin/lf --preview
```

Continue only on `VERDICT: promote (no migration to apply)`. `promote and apply
pending migration` or `REFUSED` means the build carries schema work, so stop.

```bash
./target/release/lf self install promote --cli-target ~/.local/bin/lf
```

Rerun step 1's `active.json` command: it now prints the hotfix commit and the
new active CLI path.

Desktop's bundled helper (`/Applications/Loopflow.app/Contents/MacOS/lf`) is
not part of a CLI promote. Name it as stale in the report.

## 6. Roll over stale processes

Running lf processes keep their binary until they exit. Count them by binary:

```bash
ps -axo comm= | grep -E '/\.lf/bin/lf-|Loopflow\.app/Contents/MacOS/lf' | sort | uniq -c
```

Anything not on the active CLI path from step 5 is stale. Until LOO-634 ships
a rollover command, move only what is safe to stop by hand:

- Sessions that are available (`lf session list --available --all --json`) and
  have no running Graph can be stopped: `kill` the stale `lf session serve`
  process that hosts them. They resume on the new binary with
  `lf resume`.
- Leave busy Sessions and running Graphs alone. They pick up the fix when they
  exit. List them in the report with their binary.
- Quit and reopen Desktop only when the person using it agrees.

Report the counts by binary before and after.

## 7. PR to main, noted on the release in flight

The hotfix branch is based on a release, so carry the commit to main on its own
branch:

```bash
git fetch origin main
git switch -c <user>/hotfix-<slug>-main origin/main
git cherry-pick <hotfix sha>
lf pr publish
```

Resolve conflicts against main's code, rerun step 3's focused test there, and
remeasure on a new clone if the fixed path changed shape.

Then check for a release that would regress the hotfix: an open PR titled
`release: v…`, or a tag newer than the installed one that lacks the commit
(`git tag --contains <hotfix sha>`). Comment on it with the main PR's link and
whether the release must wait for it.

## Report

- Base revision and hotfix commit, the build's `hotfix <rev> on <tag>` label,
  and the empty migration diff.
- Clone measurement: statement, cold/warm before and after, query plan.
- Focused tests: command and result, run before the promote.
- Preview verdict, and the active revision after the promote.
- Stale processes by binary, before and after rollover, plus what was left
  running and why.
- PR to main, and the release PR or tag it was noted on.
