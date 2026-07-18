#!/usr/bin/env bash
set -euo pipefail

unset LOOPFLOW_DIRECTIVE_FILE LF_WORKTREE_WRITER_ID LF_GIT_OPERATION_ID

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
LF_BIN="$ROOT_DIR/target/debug/lf"
TMP_ROOT=$(mktemp -d)

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
printf '%s %s %s\n' "${SENTINEL_MODE:-unknown}" "${LF_WORKTREE_WRITER_ID:-missing}" "${LF_GIT_OPERATION_ID:-missing}" >>"$SENTINEL_LOG"
case "${SENTINEL_MODE:-noop}" in
  resolve)
    printf 'resolved by owned recovery\n' >conflict.txt
    "$LF_TEST_BIN" rebase --continue
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
  nested_rebase)
    "$LF_TEST_BIN" rebase
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

# Clean mechanical rebase: an unavailable sentinel would fail the scenario if
# the provider launch seam were touched.
create_clean_repo clean
export SENTINEL_MODE=noop SENTINEL_LOG="$TMP_ROOT/clean.log"
: >"$SENTINEL_LOG"
(cd "$REPO" && "$LF_BIN" rebase >/dev/null)
test ! -s "$SENTINEL_LOG"
echo "PASS clean rebase used no provider"

# Publishing a deliberately behind branch pushes and updates the review surface
# without entering the integration path. A later explicit rebase owns that work.
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
(cd "$REPO" && "$LF_BIN" rebase >/dev/null)
git -C "$REPO" merge-base --is-ancestor origin/main HEAD
test "$(git -C "$REPO" rev-parse HEAD)" != "$published_head"
echo "PASS publication stayed integration-free until explicit rebase"

# A provider-owned writer id is reentrant for nested Loopflow integration.
create_clean_repo nested
export SENTINEL_MODE=nested_rebase SENTINEL_LOG="$TMP_ROOT/nested.log"
: >"$SENTINEL_LOG"
(cd "$REPO" && "$LF_BIN" : run-nested-rebase >/dev/null)
test "$(wc -l <"$SENTINEL_LOG" | tr -d ' ')" = 1
grep -Eq '^nested_rebase writer_[^ ]+ missing$' "$SENTINEL_LOG"
test "$(git --git-dir="$REMOTE" rev-parse refs/heads/feature)" = "$(git -C "$REPO" rev-parse HEAD)"

# A provider holding the worktree writer claim excludes integration before Git
# moves. This uses the real launch boundary rather than a public test-only lease.
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
set +e
(cd "$REPO" && "$LF_BIN" rebase >"$TMP_ROOT/writer.rebase.out" 2>&1)
writer_rebase_status=$?
set -e
test "$writer_rebase_status" -ne 0
grep -q 'independent writer' "$TMP_ROOT/writer.rebase.out"
test "$(git -C "$REPO" rev-parse HEAD)" = "$writer_head"
test ! -e "$(git -C "$REPO" rev-parse --absolute-git-dir)/loopflow/rebase-owner.json"
: >"$SENTINEL_RELEASE"
set +e
wait "$writer_owner"
set -e
echo "PASS independent agent writer blocked integration before Git moved"

# Hold the first recovery provider open. Foreign rebase and skill invocations
# must refuse while preserving the exact conflict and launching no second agent.
create_conflict_repo foreign
export SENTINEL_MODE=hold SENTINEL_LOG="$TMP_ROOT/foreign.log"
export SENTINEL_READY="$TMP_ROOT/foreign.ready" SENTINEL_RELEASE="$TMP_ROOT/foreign.release"
export SENTINEL_PID_FILE="$TMP_ROOT/sentinel.pid"
: >"$SENTINEL_LOG"
(cd "$REPO" && exec "$LF_BIN" rebase >"$TMP_ROOT/foreign.owner.out" 2>&1) &
foreign_owner=$!
for _ in $(seq 1 200); do [ -e "$SENTINEL_READY" ] && break; sleep 0.05; done
test -e "$SENTINEL_READY"
owned_head=$(git -C "$REPO" rev-parse HEAD)
set +e
(cd "$REPO" && "$LF_BIN" rebase >"$TMP_ROOT/foreign.rebase.out" 2>&1)
foreign_rebase_status=$?
(cd "$REPO" && "$LF_BIN" implement >"$TMP_ROOT/foreign.agent.out" 2>&1)
foreign_agent_status=$?
(cd "$REPO" && LF_GIT_OPERATION_ID=gitop_foreign "$LF_BIN" rebase --continue >"$TMP_ROOT/foreign.continue.out" 2>&1)
foreign_continue_status=$?
(cd "$REPO" && LF_GIT_OPERATION_ID=gitop_foreign "$LF_BIN" rebase --abort >"$TMP_ROOT/foreign.abort.out" 2>&1)
foreign_abort_status=$?
set -e
test "$foreign_rebase_status" -ne 0
test "$foreign_agent_status" -ne 0
test "$foreign_continue_status" -ne 0
test "$foreign_abort_status" -ne 0
test "$(git -C "$REPO" rev-parse HEAD)" = "$owned_head"
test -d "$(git -C "$REPO" rev-parse --absolute-git-dir)/rebase-merge" -o \
  -d "$(git -C "$REPO" rev-parse --absolute-git-dir)/rebase-apply"
test "$(wc -l <"$SENTINEL_LOG" | tr -d ' ')" = 1
: >"$SENTINEL_RELEASE"
set +e
wait "$foreign_owner"
set -e
echo "PASS foreign rebase and agent launch changed no owned state"

# A matching recovery child continues the sequencer it inherited. The parent
# verifies and pushes once after the sentinel exits.
create_conflict_repo authorized
authorized_original=$(git -C "$REPO" rev-parse HEAD)
export SENTINEL_MODE=resolve SENTINEL_LOG="$TMP_ROOT/authorized.log"
: >"$SENTINEL_LOG"
(cd "$REPO" && "$LF_BIN" rebase >/dev/null)
test "$(wc -l <"$SENTINEL_LOG" | tr -d ' ')" = 1
grep -Eq '^resolve writer_[^ ]+ gitop_[^ ]+$' "$SENTINEL_LOG"
test "$(git --git-dir="$REMOTE" rev-parse refs/heads/feature)" = "$(git -C "$REPO" rev-parse HEAD)"
test ! -d "$(git -C "$REPO" rev-parse --absolute-git-dir)/rebase-merge"
test ! -d "$(git -C "$REPO" rev-parse --absolute-git-dir)/rebase-apply"
echo "PASS authorized recovery continued the original sequencer"

# Replaying the exact conflict reuses the reviewed resolution mechanically.
# rerere never auto-stages: Loopflow stages only the reused unmerged paths.
git -C "$REPO" reset --hard "$authorized_original" >/dev/null
: >"$SENTINEL_LOG"
printf 'unrelated\n' >"$REPO/unrelated.tmp"
(cd "$REPO" && "$LF_BIN" rebase >/dev/null)
test ! -s "$SENTINEL_LOG"
test "$(cat "$REPO/conflict.txt")" = "resolved by owned recovery"
test -f "$REPO/unrelated.tmp"
test -z "$(git -C "$REPO" diff --cached --name-only)"
echo "PASS repeated conflict reused resolution without a provider"

# Land recovery resumes after the verified integration instead of replaying the
# collapse/rebase path and pushing a second time.
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
(cd "$REPO" && "$LF_BIN" pr land --create-pr --title "one replay" --body "proof" >/dev/null)
test "$(wc -l <"$SENTINEL_LOG" | tr -d ' ')" = 1
test "$(grep -c '^refs/heads/feature$' "$land_push_log")" = 1
test "$(git -C "$REPO" rev-list --count origin/main..HEAD)" = 1
test -f "$REPO/first.txt"
test ! -e "$REPO/scratch/working.md"
test -z "$(git -C "$REPO" diff --name-only origin/main...HEAD -- scratch)"
echo "PASS recovered land integrated and pushed once"

# Provider success without Git success fails the operation and retains both the
# sequencer and descriptive owner metadata for explicit recovery.
create_conflict_repo incomplete
export SENTINEL_MODE=noop SENTINEL_LOG="$TMP_ROOT/incomplete.log"
: >"$SENTINEL_LOG"
set +e
(cd "$REPO" && "$LF_BIN" rebase >"$TMP_ROOT/incomplete.out" 2>&1)
incomplete_status=$?
set -e
test "$incomplete_status" -ne 0
grep -q 'still reports an active rebase operation' "$TMP_ROOT/incomplete.out"
test -f "$(git -C "$REPO" rev-parse --absolute-git-dir)/loopflow/rebase-owner.json"
echo "PASS zero-exit incomplete recovery failed and remained recoverable"

# Kill the owner and race two explicit continuations. The stale lock can be
# claimed once; the loser observes either the new live owner or completed state.
create_conflict_repo stale
export SENTINEL_MODE=hold SENTINEL_LOG="$TMP_ROOT/stale.log"
export SENTINEL_READY="$TMP_ROOT/stale.ready" SENTINEL_RELEASE="$TMP_ROOT/stale.release"
export SENTINEL_PID_FILE="$TMP_ROOT/sentinel.pid"
: >"$SENTINEL_LOG"
(cd "$REPO" && exec "$LF_BIN" rebase >"$TMP_ROOT/stale.owner.out" 2>&1) &
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
(cd "$REPO" && "$LF_BIN" rebase --continue >"$TMP_ROOT/adopt.one" 2>&1) &
adopt_one=$!
(cd "$REPO" && "$LF_BIN" rebase --continue >"$TMP_ROOT/adopt.two" 2>&1) &
adopt_two=$!
wait "$adopt_one"; adopt_one_status=$?
wait "$adopt_two"; adopt_two_status=$?
set -e
test $(( (adopt_one_status == 0) + (adopt_two_status == 0) )) -eq 1
echo "PASS stale ownership was adopted exactly once"

# Private Git dirs permit two linked worktrees to own and resolve independent
# rebases concurrently.
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
(cd "$TMP_ROOT/linked.one" && "$LF_BIN" rebase >"$TMP_ROOT/linked.one.out" 2>&1) &
linked_one=$!
(cd "$TMP_ROOT/linked.two" && "$LF_BIN" rebase >"$TMP_ROOT/linked.two.out" 2>&1) &
linked_two=$!
wait "$linked_one"
wait "$linked_two"
test "$(wc -l <"$SENTINEL_LOG" | tr -d ' ')" = 2
echo "PASS linked worktrees rebased independently"
