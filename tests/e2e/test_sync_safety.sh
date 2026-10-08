#!/usr/bin/env bash
set -euo pipefail

unset LOOPFLOW_DIRECTIVE_FILE LF_GIT_OPERATION_ID

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
LF_BIN="$ROOT_DIR/target/debug/lf"
TMP_ROOT=$(mktemp -d)
export LF_HOME="$TMP_ROOT/lf-home"
unset LF_RUN_ID LF_RUN_DIR LF_WORK_ADVANCE_CLAIM LF_HUMAN_SESSION

cleanup() {
  find "$TMP_ROOT" -name sentinel.pid -type f -print0 2>/dev/null |
    xargs -0 -I{} sh -c 'kill "$(cat "$1")" 2>/dev/null || true' sh {} || true
  rm -rf "$TMP_ROOT"
}
trap cleanup EXIT

cargo build --quiet --manifest-path "$ROOT_DIR/Cargo.toml" -p loopflow --bin lf

BIN_DIR="$TMP_ROOT/bin"
mkdir -p "$BIN_DIR"
cat >"$BIN_DIR/opencode" <<'SENTINEL'
#!/usr/bin/env bash
set -euo pipefail
if [ "${1:-}" = "--version" ]; then
  echo "opencode sentinel"
  exit 0
fi
printf '%s %s\n' "${SENTINEL_MODE:-unknown}" "${LF_GIT_OPERATION_ID:-missing}" >>"$SENTINEL_LOG"
case "${SENTINEL_MODE:-noop}" in
  resolve|adopt)
    printf 'resolved by owned recovery\n' >conflict.txt
    args=(sync --continue)
    if [ "$SENTINEL_MODE" = adopt ]; then args+=(--adopt); fi
    "$LF_TEST_BIN" "${args[@]}"
    ;;
  hold)
    echo $$ >"$SENTINEL_PID_FILE"
    : >"$SENTINEL_READY"
    trap 'exit 143' TERM INT
    while [ ! -e "$SENTINEL_RELEASE" ]; do sleep 0.05; done
    exit 7
    ;;
  noop)
    exit 0
    ;;
  nested_sync)
    "$LF_TEST_BIN" sync
    ;;
esac
SENTINEL
chmod +x "$BIN_DIR/opencode"
export PATH="$BIN_DIR:$PATH"
export LF_TEST_BIN="$LF_BIN"

configure_repo() {
  local repo=$1
  git -C "$repo" config user.email "loopflow@example.com"
  git -C "$repo" config user.name "Loopflow"
  mkdir -p "$repo/.lf"
  printf 'agent: opencode\n' >"$repo/.lf/config.yaml"
}

create_conflict_repo() {
  local name=$1
  REMOTE="$TMP_ROOT/$name.git"
  REPO="$TMP_ROOT/$name"
  OTHER="$TMP_ROOT/$name.other"
  git init --bare -b main "$REMOTE" >/dev/null
  git clone "$REMOTE" "$REPO" >/dev/null
  configure_repo "$REPO"
  mkdir -p "$REPO/scratch"
  : >"$REPO/scratch/.gitkeep"
  printf 'base\n' >"$REPO/conflict.txt"
  git -C "$REPO" add conflict.txt scratch/.gitkeep
  git -C "$REPO" commit -m base >/dev/null
  git -C "$REPO" push -u origin main >/dev/null
  git -C "$REPO" checkout -b feature >/dev/null
  printf 'feature\n' >"$REPO/conflict.txt"
  git -C "$REPO" add conflict.txt
  git -C "$REPO" commit -m feature >/dev/null
  git -C "$REPO" push -u origin feature >/dev/null

  git clone "$REMOTE" "$OTHER" >/dev/null
  configure_repo "$OTHER"
  git -C "$OTHER" checkout main >/dev/null
  printf 'main\n' >"$OTHER/conflict.txt"
  git -C "$OTHER" add conflict.txt
  git -C "$OTHER" commit -m main >/dev/null
  git -C "$OTHER" push origin main >/dev/null
}

create_clean_repo() {
  local name=$1
  REMOTE="$TMP_ROOT/$name.git"
  REPO="$TMP_ROOT/$name"
  OTHER="$TMP_ROOT/$name.other"
  git init --bare -b main "$REMOTE" >/dev/null
  git clone "$REMOTE" "$REPO" >/dev/null
  configure_repo "$REPO"
  printf 'base\n' >"$REPO/base.txt"
  git -C "$REPO" add base.txt
  git -C "$REPO" commit -m base >/dev/null
  git -C "$REPO" push -u origin main >/dev/null
  git -C "$REPO" checkout -b feature >/dev/null
  printf 'feature\n' >"$REPO/feature.txt"
  git -C "$REPO" add feature.txt
  git -C "$REPO" commit -m feature >/dev/null
  git -C "$REPO" push -u origin feature >/dev/null

  git clone "$REMOTE" "$OTHER" >/dev/null
  configure_repo "$OTHER"
  git -C "$OTHER" checkout main >/dev/null
  printf 'main\n' >"$OTHER/main.txt"
  git -C "$OTHER" add main.txt
  git -C "$OTHER" commit -m main >/dev/null
  git -C "$OTHER" push origin main >/dev/null
}

# Clean mechanical sync: an unavailable sentinel would fail the scenario if
# the provider launch seam were touched.
create_clean_repo clean
export SENTINEL_MODE=noop SENTINEL_LOG="$TMP_ROOT/clean.log"
: >"$SENTINEL_LOG"
(cd "$REPO" && "$LF_BIN" sync >/dev/null)
test ! -s "$SENTINEL_LOG"
echo "PASS clean sync used no provider"

# A supervisor can hand off a raw merge, and a stopped Loopflow sync keeps its
# pinned target for the next agent. Launch itself neither claims nor publishes.
for merge_owner in raw stopped; do
  create_conflict_repo "handoff-$merge_owner"
  git -C "$REPO" fetch origin main >/dev/null
  handoff_head=$(git -C "$REPO" rev-parse HEAD)
  handoff_target=$(git -C "$REPO" rev-parse origin/main)
  export SENTINEL_LOG="$TMP_ROOT/handoff-$merge_owner.log"
  : >"$SENTINEL_LOG"
  set +e
  if [ "$merge_owner" = raw ]; then
    git -C "$REPO" merge origin/main >"$TMP_ROOT/handoff-$merge_owner.out" 2>&1
  else
    (cd "$REPO" && "$LF_BIN" sync --manual >"$TMP_ROOT/handoff-$merge_owner.out" 2>&1)
  fi
  handoff_status=$?
  set -e
  test "$handoff_status" -ne 0
  export SENTINEL_MODE=resolve
  if [ "$merge_owner" = raw ]; then export SENTINEL_MODE=adopt; fi
  (cd "$REPO" && "$LF_BIN" : finish-existing-merge >/dev/null)
  test "$(git -C "$REPO" show -s --format=%P HEAD)" = "$handoff_head $handoff_target"
  test "$(git --git-dir="$REMOTE" rev-parse refs/heads/feature)" = "$handoff_head"
  test "$(cat "$REPO/conflict.txt")" = "resolved by owned recovery"
  test ! -f "$(git -C "$REPO" rev-parse --absolute-git-dir)/MERGE_HEAD"
  test ! -f "$(git -C "$REPO" rev-parse --absolute-git-dir)/loopflow/rebase-owner.json"
  test "$(wc -l <"$SENTINEL_LOG" | tr -d ' ')" = 1
  echo "PASS agent completed $merge_owner merge handoff locally"
done

# A deleted remote branch leaves tracking behind; the CLI must recreate it
# after merging main and leave its upstream usable on the next invocation.
create_clean_repo deleted
published_head=$(git -C "$REPO" rev-parse origin/feature)
git --git-dir="$REMOTE" update-ref -d refs/heads/feature
test "$(git -C "$REPO" rev-parse origin/feature)" = "$published_head"
(cd "$REPO" && "$LF_BIN" sync >/dev/null)
git -C "$REPO" merge-base --is-ancestor origin/main HEAD
test "$(git --git-dir="$REMOTE" rev-parse refs/heads/feature)" = "$(git -C "$REPO" rev-parse HEAD)"
test "$(git --git-dir="$REMOTE" show feature:feature.txt)" = feature
test "$(git --git-dir="$REMOTE" show feature:main.txt)" = main
(cd "$REPO" && "$LF_BIN" sync >/dev/null)
test "$(git -C "$REPO" rev-parse '@{upstream}')" = "$(git -C "$REPO" rev-parse HEAD)"
echo "PASS deleted remote branch recreated with usable tracking"

# Publishing a deliberately behind branch pushes and updates the review surface
# without entering the integration path. A later explicit sync owns that work.
create_clean_repo publication
pre_publish_head=$(git -C "$REPO" rev-parse HEAD)
cat >"$BIN_DIR/gh" <<'GH_SENTINEL'
#!/usr/bin/env bash
set -euo pipefail
if [ "${1:-}" = "--version" ]; then exit 0; fi
printf '%s\n' "$*" >>"$GH_LOG"
case "${1:-} ${2:-}" in
  "pr list")
    if [ -e "$GH_STATE" ]; then
      printf '[{"url":"https://example.com/pr/7","state":"OPEN","isDraft":false,"number":7,"mergeCommit":null}]\n'
    else
      printf '[]\n'
    fi
    ;;
  "pr create")
    : >"$GH_STATE"
    printf 'https://example.com/pr/7\n'
    ;;
  "api graphql")
    request=null
    if [ -e "$GH_STATE.auto" ]; then request='{"enabledAt":"2026-09-30T00:00:00Z"}'; fi
    printf '{"data":{"repository":{"pullRequest":{"id":"PR_fixture","number":7,"url":"https://example.com/pr/7","state":"OPEN","isDraft":false,"headRefName":"feature","headRefOid":"%s","mergedAt":null,"mergeCommit":null,"mergeStateStatus":"CLEAN","isMergeQueueEnabled":false,"autoMergeRequest":%s,"mergeQueueEntry":null}}}}\n' "$(git rev-parse HEAD)" "$request"
    ;;
  "pr merge")
    if [[ "$*" == *--disable-auto* ]]; then rm -f "$GH_STATE.auto"; else : >"$GH_STATE.auto"; fi
    ;;
  "pr edit"|"pr ready") ;;
esac
GH_SENTINEL
cat >"$BIN_DIR/open" <<'OPEN_SENTINEL'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$OPEN_LOG"
OPEN_SENTINEL
cp "$BIN_DIR/open" "$BIN_DIR/xdg-open"
chmod +x "$BIN_DIR/gh" "$BIN_DIR/open" "$BIN_DIR/xdg-open"
export GH_LOG="$TMP_ROOT/publication.gh.log" GH_STATE="$TMP_ROOT/publication.gh.state"
export OPEN_LOG="$TMP_ROOT/publication.open.log"
export SENTINEL_MODE=noop SENTINEL_LOG="$TMP_ROOT/publication.provider.log"
: >"$GH_LOG"; : >"$OPEN_LOG"; : >"$SENTINEL_LOG"
(cd "$REPO" && "$LF_BIN" pr publish --title "behind branch" --body "proof" >/dev/null)
published_head=$(git -C "$REPO" rev-parse HEAD)
test "$(git -C "$REPO" rev-list --count "$pre_publish_head..$published_head")" = 1
test "$(git --git-dir="$REMOTE" rev-parse refs/heads/feature)" = "$published_head"
test ! -e "$(git -C "$REPO" rev-parse --absolute-git-dir)/loopflow/rebase-owner.json"
test ! -s "$SENTINEL_LOG"
(cd "$REPO" && "$LF_BIN" pr open --title "behind branch" --body "proof" >/dev/null)
test "$(git -C "$REPO" rev-parse HEAD)" = "$published_head"
test "$(wc -l <"$OPEN_LOG" | tr -d ' ')" = 1
test ! -s "$SENTINEL_LOG"
(cd "$REPO" && "$LF_BIN" sync >/dev/null)
git -C "$REPO" merge-base --is-ancestor origin/main HEAD
test "$(git -C "$REPO" rev-parse HEAD)" != "$published_head"
echo "PASS publication stayed integration-free until explicit sync"

# A provider receives no worktree authority; nested integration takes its own
# short mutation lock.
create_clean_repo nested
export SENTINEL_MODE=nested_sync SENTINEL_LOG="$TMP_ROOT/nested.log"
: >"$SENTINEL_LOG"
(cd "$REPO" && "$LF_BIN" : run-nested-sync >/dev/null)
test "$(wc -l <"$SENTINEL_LOG" | tr -d ' ')" = 1
grep -Eq '^nested_sync missing$' "$SENTINEL_LOG"
test "$(git --git-dir="$REMOTE" rev-parse refs/heads/feature)" = "$(git -C "$REPO" rev-parse HEAD)"

# A live provider does not reserve the worktree. A separate sync can take the
# short mutation lock and complete while the provider remains alive.
create_clean_repo writer
export SENTINEL_MODE=hold SENTINEL_LOG="$TMP_ROOT/writer.log"
export SENTINEL_READY="$TMP_ROOT/writer.ready" SENTINEL_RELEASE="$TMP_ROOT/writer.release"
export SENTINEL_PID_FILE="$TMP_ROOT/sentinel.pid"
: >"$SENTINEL_LOG"
(cd "$REPO" && exec "$LF_BIN" : hold-writer >"$TMP_ROOT/writer.owner.out" 2>&1) &
writer_owner=$!
for _ in $(seq 1 200); do [ -e "$SENTINEL_READY" ] && break; sleep 0.05; done
test -e "$SENTINEL_READY"
writer_head=$(git -C "$REPO" rev-parse HEAD)
(cd "$REPO" && "$LF_BIN" sync >"$TMP_ROOT/writer.sync.out" 2>&1)
writer_sync_status=$?
test "$writer_sync_status" -eq 0
test "$(git -C "$REPO" rev-parse HEAD)" != "$writer_head"
test "$(git --git-dir="$REMOTE" rev-parse refs/heads/feature)" = "$(git -C "$REPO" rev-parse HEAD)"
test ! -e "$(git -C "$REPO" rev-parse --absolute-git-dir)/loopflow/rebase-owner.json"
: >"$SENTINEL_RELEASE"
set +e
wait "$writer_owner"
set -e
echo "PASS live provider held no worktree authority"

# Hold the first recovery provider open. Foreign sync and skill invocations
# must refuse while preserving the exact conflict and launching no second agent.
create_conflict_repo foreign
export SENTINEL_MODE=hold SENTINEL_LOG="$TMP_ROOT/foreign.log"
export SENTINEL_READY="$TMP_ROOT/foreign.ready" SENTINEL_RELEASE="$TMP_ROOT/foreign.release"
export SENTINEL_PID_FILE="$TMP_ROOT/sentinel.pid"
: >"$SENTINEL_LOG"
(cd "$REPO" && exec "$LF_BIN" sync >"$TMP_ROOT/foreign.owner.out" 2>&1) &
foreign_owner=$!
for _ in $(seq 1 200); do [ -e "$SENTINEL_READY" ] && break; sleep 0.05; done
test -e "$SENTINEL_READY"
owned_head=$(git -C "$REPO" rev-parse HEAD)
set +e
(cd "$REPO" && "$LF_BIN" sync >"$TMP_ROOT/foreign.sync.out" 2>&1)
foreign_sync_status=$?
(cd "$REPO" && "$LF_BIN" implement >"$TMP_ROOT/foreign.agent.out" 2>&1)
foreign_agent_status=$?
(cd "$REPO" && LF_GIT_OPERATION_ID=gitop_foreign "$LF_BIN" sync --continue >"$TMP_ROOT/foreign.continue.out" 2>&1)
foreign_continue_status=$?
(cd "$REPO" && LF_GIT_OPERATION_ID=gitop_foreign "$LF_BIN" sync --abort >"$TMP_ROOT/foreign.abort.out" 2>&1)
foreign_abort_status=$?
set -e
test "$foreign_sync_status" -ne 0
test "$foreign_agent_status" -ne 0
test "$foreign_continue_status" -ne 0
test "$foreign_abort_status" -ne 0
test "$(git -C "$REPO" rev-parse HEAD)" = "$owned_head"
test -f "$(git -C "$REPO" rev-parse --absolute-git-dir)/MERGE_HEAD"
test "$(wc -l <"$SENTINEL_LOG" | tr -d ' ')" = 1
: >"$SENTINEL_RELEASE"
set +e
wait "$foreign_owner"
set -e
echo "PASS foreign sync and agent launch changed no owned state"

# A matching recovery child continues the sequencer it inherited. The parent
# verifies and pushes once after the sentinel exits.
create_conflict_repo authorized
authorized_original=$(git -C "$REPO" rev-parse HEAD)
export SENTINEL_MODE=resolve SENTINEL_LOG="$TMP_ROOT/authorized.log"
: >"$SENTINEL_LOG"
(cd "$REPO" && "$LF_BIN" sync >/dev/null)
test "$(wc -l <"$SENTINEL_LOG" | tr -d ' ')" = 1
grep -Eq '^resolve gitop_[^ ]+$' "$SENTINEL_LOG"
test "$(git --git-dir="$REMOTE" rev-parse refs/heads/feature)" = "$(git -C "$REPO" rev-parse HEAD)"
test ! -f "$(git -C "$REPO" rev-parse --absolute-git-dir)/MERGE_HEAD"

echo "PASS authorized recovery continued the original sequencer"

# Replaying the exact conflict reuses the reviewed resolution mechanically.
# rerere never auto-stages: Loopflow stages only the reused unmerged paths.
git -C "$REPO" reset --hard "$authorized_original" >/dev/null
: >"$SENTINEL_LOG"
printf 'unrelated\n' >"$REPO/unrelated.tmp"
(cd "$REPO" && "$LF_BIN" sync >/dev/null)
test ! -s "$SENTINEL_LOG"
test "$(cat "$REPO/conflict.txt")" = "resolved by owned recovery"
test -f "$REPO/unrelated.tmp"
test -z "$(git -C "$REPO" diff --cached --name-only)"
echo "PASS repeated conflict reused resolution without a provider"

# The shared arm/land preparation resumes after the verified integration instead
# of starting another merge and pushing twice. Watcher proof is in land_tests.
create_conflict_repo land-recovery
printf 'checkpoint one\n' >"$REPO/first.txt"
git -C "$REPO" add first.txt
git -C "$REPO" commit -m 'checkpoint: first slice' >/dev/null
mkdir -p "$REPO/scratch"
printf 'discard me\n' >"$REPO/scratch/working.md"
git -C "$REPO" add scratch/working.md
git -C "$REPO" commit -m 'checkpoint: working notes' >/dev/null
git -C "$REPO" push origin feature >/dev/null
land_push_log="$TMP_ROOT/land-recovery.push.log"
cat >"$REMOTE/hooks/update" <<PUSH_HOOK
#!/usr/bin/env bash
printf '%s\n' "\$1" >>"$land_push_log"
PUSH_HOOK
chmod +x "$REMOTE/hooks/update"
export SENTINEL_MODE=resolve SENTINEL_LOG="$TMP_ROOT/land-recovery.provider.log"
: >"$SENTINEL_LOG"; : >"$land_push_log"; rm -f "$GH_STATE"
(cd "$REPO" && "$LF_BIN" pr arm --title "one merge" --body "proof" >/dev/null)
test "$(wc -l <"$SENTINEL_LOG" | tr -d ' ')" = 1
test "$(grep -c '^refs/heads/feature$' "$land_push_log")" = 1
git -C "$REPO" log --format=%s origin/main..HEAD | grep -q "checkpoint: first slice"
test -f "$REPO/first.txt"
test ! -e "$REPO/scratch/working.md"
test -z "$(git -C "$REPO" diff --name-only origin/main...HEAD -- scratch)"
echo "PASS recovered arm integrated and pushed once"

# Provider success without Git success fails the operation and retains both the
# sequencer and descriptive owner metadata for explicit recovery.
create_conflict_repo incomplete
export SENTINEL_MODE=noop SENTINEL_LOG="$TMP_ROOT/incomplete.log"
: >"$SENTINEL_LOG"
set +e
(cd "$REPO" && "$LF_BIN" sync >"$TMP_ROOT/incomplete.out" 2>&1)
incomplete_status=$?
set -e
test "$incomplete_status" -ne 0
grep -q 'still reports an active merge operation' "$TMP_ROOT/incomplete.out"
test -f "$(git -C "$REPO" rev-parse --absolute-git-dir)/loopflow/rebase-owner.json"
echo "PASS zero-exit incomplete recovery failed and remained recoverable"

# Kill the owner and race two explicit continuations. The stale lock can be
# claimed once; the loser observes either the new live owner or completed state.
create_conflict_repo stale
export SENTINEL_MODE=hold SENTINEL_LOG="$TMP_ROOT/stale.log"
export SENTINEL_READY="$TMP_ROOT/stale.ready" SENTINEL_RELEASE="$TMP_ROOT/stale.release"
export SENTINEL_PID_FILE="$TMP_ROOT/sentinel.pid"
: >"$SENTINEL_LOG"
(cd "$REPO" && exec "$LF_BIN" sync >"$TMP_ROOT/stale.owner.out" 2>&1) &
stale_owner=$!
for _ in $(seq 1 200); do [ -e "$SENTINEL_READY" ] && break; sleep 0.05; done
test -e "$SENTINEL_READY"
kill -TERM "$stale_owner" 2>/dev/null || true
set +e
wait "$stale_owner"
set -e
if [ -f "$SENTINEL_PID_FILE" ]; then
  kill "$(cat "$SENTINEL_PID_FILE")" 2>/dev/null || true
fi
printf 'resolved after owner death\n' >"$REPO/conflict.txt"
set +e
(cd "$REPO" && "$LF_BIN" sync --continue >"$TMP_ROOT/adopt.one" 2>&1) &
adopt_one=$!
(cd "$REPO" && "$LF_BIN" sync --continue >"$TMP_ROOT/adopt.two" 2>&1) &
adopt_two=$!
wait "$adopt_one"; adopt_one_status=$?
wait "$adopt_two"; adopt_two_status=$?
set -e
test $(( (adopt_one_status == 0) + (adopt_two_status == 0) )) -eq 1
echo "PASS stale ownership was adopted exactly once"

# Private Git dirs permit two linked worktrees to own and resolve independent
# syncs concurrently.
REMOTE="$TMP_ROOT/linked.git"
REPO="$TMP_ROOT/linked"
git init --bare -b main "$REMOTE" >/dev/null
git clone "$REMOTE" "$REPO" >/dev/null
configure_repo "$REPO"
printf 'base\n' >"$REPO/conflict.txt"
git -C "$REPO" add conflict.txt
git -C "$REPO" commit -m base >/dev/null
git -C "$REPO" push -u origin main >/dev/null
git -C "$REPO" worktree add -b feature-one "$TMP_ROOT/linked.one" main >/dev/null
git -C "$REPO" worktree add -b feature-two "$TMP_ROOT/linked.two" main >/dev/null
configure_repo "$TMP_ROOT/linked.one"
configure_repo "$TMP_ROOT/linked.two"
printf 'one\n' >"$TMP_ROOT/linked.one/conflict.txt"
git -C "$TMP_ROOT/linked.one" add conflict.txt
git -C "$TMP_ROOT/linked.one" commit -m one >/dev/null
git -C "$TMP_ROOT/linked.one" push -u origin feature-one >/dev/null
printf 'two\n' >"$TMP_ROOT/linked.two/conflict.txt"
git -C "$TMP_ROOT/linked.two" add conflict.txt
git -C "$TMP_ROOT/linked.two" commit -m two >/dev/null
git -C "$TMP_ROOT/linked.two" push -u origin feature-two >/dev/null
printf 'main\n' >"$REPO/conflict.txt"
git -C "$REPO" add conflict.txt
git -C "$REPO" commit -m main >/dev/null
git -C "$REPO" push origin main >/dev/null
export SENTINEL_MODE=resolve SENTINEL_LOG="$TMP_ROOT/linked.log"
: >"$SENTINEL_LOG"
(cd "$TMP_ROOT/linked.one" && "$LF_BIN" sync >"$TMP_ROOT/linked.one.out" 2>&1) &
linked_one=$!
(cd "$TMP_ROOT/linked.two" && "$LF_BIN" sync >"$TMP_ROOT/linked.two.out" 2>&1) &
linked_two=$!
wait "$linked_one"
wait "$linked_two"
test "$(wc -l <"$SENTINEL_LOG" | tr -d ' ')" = 2
echo "PASS linked worktrees synced independently"
