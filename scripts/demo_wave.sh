#!/usr/bin/env bash
# demo_wave.sh — guided live demo of the wave (`lf serve`), two processes:
# the LISTENER (journal pens, doors, supervisor — vendor-free) and the
# RESIDENT it spawns (`lf __resident`, with private server env,
# running in the wave's own worktree).
#
# Walks the whole surface against a throwaway repo: boot + discovery (both
# processes from one command), chat, steer, interrupt, worker dispatch (real
# `lf q worker run`), attributed worker reports arriving in the thread,
# memory curation, restart with the thread intact, clean teardown.
#
# COSTS: every loop pass runs a real three-phase wave-pass (acts 2-6).
# REQUIRES: codex CLI authed (`codex login`), tmux, jq, curl.
#
# Binary resolution: $LF_BIN if set, else <repo>/target/release/lf
# (`cargo build --release`), else `lf` on PATH.
#
# Usage:
#   bash scripts/demo_wave.sh            # full guided demo, pauses between acts
#   bash scripts/demo_wave.sh --fast     # no pauses
#   bash scripts/demo_wave.sh --smoke    # setup + launch + health + teardown
#                                        # only — zero model turns
#
# Timing is model-dependent: the script polls with timeouts and tells you
# what to look for rather than asserting exact content.
set -euo pipefail

FAST=0
SMOKE=0
for arg in "$@"; do
    case "$arg" in
        --fast) FAST=1 ;;
        --smoke) SMOKE=1 ;;
        *) echo "unknown flag: $arg (try --fast or --smoke)" >&2; exit 2 ;;
    esac
done

# ---------- helpers ------------------------------------------------------

hr()      { printf '\n\033[1;35m%s\033[0m\n' "── $* ──"; }
say()     { printf '\033[0;36m%s\033[0m\n' "$*"; }
ok()      { printf '\033[0;32m✓ %s\033[0m\n' "$*"; }
warn()    { printf '\033[0;33m! %s\033[0m\n' "$*"; }

pause() {
    if [[ $FAST -eq 0 && $SMOKE -eq 0 ]]; then
        read -r -p $'\033[1m[enter to continue]\033[0m '
    fi
}

# poll "description" timeout_seconds command...
# Runs command every 2s until it exits 0 or the timeout passes. Returns the
# command's last exit code; never kills the demo (callers narrate failure).
poll() {
    local desc="$1" timeout="$2"; shift 2
    local waited=0
    while ! "$@" >/dev/null 2>&1; do
        if (( waited >= timeout )); then
            warn "timed out after ${timeout}s waiting for: $desc"
            return 1
        fi
        sleep 2; waited=$((waited + 2))
    done
    ok "$desc (${waited}s)"
}

thread() {
    curl -sf "http://$ADDR/conversation" |
        jq -r '.turns[] | [.id, .role, .status, (.from // "-"),
                           (.text | gsub("\n"; " ") | .[0:70])] | @tsv' |
        column -t -s $'\t' | sed 's/^/  /'
}

last_turn_status() { curl -sf "http://$ADDR/conversation" | jq -r '.turns[-1].status'; }
loop_state()   { curl -sf "http://$ADDR/health" | jq -r '.loop_state'; }

journal_types() {
    jq -r '.kind.type' "$JOURNAL" 2>/dev/null | sort | uniq -c | sed 's/^/  /'
}

journal_has() { jq -e -r ".kind.type" "$JOURNAL" 2>/dev/null | grep -q "^$1\$"; }

new_codex_orphans() {
    # codex processes that exist now but did not before the demo started.
    comm -13 <(echo "$CODEX_BASELINE") <(pgrep -f "codex app-server" | sort) 2>/dev/null
}

cleanup() {
    tmux kill-session -t "$TMUX_SESSION" 2>/dev/null || true
}
trap cleanup EXIT

# ---------- preflight ----------------------------------------------------

hr "preflight"
for tool in tmux jq curl git; do
    command -v "$tool" >/dev/null || { echo "missing: $tool" >&2; exit 1; }
done
command -v codex >/dev/null || { echo "missing: codex CLI (pass phases default to codex)" >&2; exit 1; }

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LF_BIN="${LF_BIN:-}"
if [[ -z "$LF_BIN" ]]; then
    if [[ -x "$REPO_ROOT/target/release/lf" ]]; then
        LF_BIN="$REPO_ROOT/target/release/lf"
    else
        LF_BIN="$(command -v lf || true)"
    fi
fi
[[ -n "$LF_BIN" && -x "$LF_BIN" ]] || {
    echo "no lf binary: set LF_BIN, or 'cargo build --release', or put lf on PATH" >&2
    exit 1
}
say "lf binary: $LF_BIN"
CODEX_BASELINE="$(pgrep -f 'codex app-server' | sort || true)"

# ---------- throwaway repo -----------------------------------------------

hr "act 0 · a throwaway repo with a demo wave"
DEMO_ROOT="$(mktemp -d /tmp/wavedemo.XXXXXX)"
DEMO_REPO="$DEMO_ROOT/demorepo"
# Unique per run: the registry keys waves by NAME alone, so a reused "demo"
# would join an older demo wave's row — inheriting its worker observations
# and colliding with a concurrent run's one-brain enforcement.
WAVE="demo-$(date +%H%M%S)"
mkdir -p "$DEMO_REPO/wave/$WAVE"
git -C "$DEMO_ROOT" >/dev/null 2>&1 || true
git init -q -b main "$DEMO_REPO"
git -C "$DEMO_REPO" config user.name "wave-demo"
git -C "$DEMO_REPO" config user.email "demo@loopflow.studio"

cat > "$DEMO_REPO/wave/$WAVE/GOAL.md" <<'EOF'
---
workers: 1
---

## Objective

Maintain TODO.md in this repo. Each pass, make ONE small, concrete
improvement: complete an item, tighten wording, or add a genuinely useful
task. Keep the file short. Commit your work.

## Measures

- **Quality**: TODO.md stays short, current, and useful.

## Process

Use the implement flow for each small improvement.
EOF
cat > "$DEMO_REPO/wave/$WAVE/MEMORY.md" <<'EOF'
# Memory

- This is a demo repo; keep every change small.
EOF
cat > "$DEMO_REPO/TODO.md" <<'EOF'
# TODO

- [ ] Write a one-line project description in README.md
- [ ] Add a .gitignore
EOF
git -C "$DEMO_REPO" add -A
git -C "$DEMO_REPO" commit -qm "seed demo wave"
say "repo: $DEMO_REPO"
say "wave: wave/$WAVE/GOAL.md (maintain TODO.md, one small improvement per pass)"
say "note: the server registers a '$WAVE' wave row in this machine's registry (~/.lf)"

ENDPOINT_FILE="$DEMO_REPO/wave/$WAVE/.wave-endpoint"
JOURNAL="$DEMO_REPO/.lf/journal/waves/$WAVE/journal.jsonl"
TMUX_SESSION="wavedemo-$$"
pause

# ---------- launch --------------------------------------------------------

hr "launch · lf serve $WAVE (detached tmux: $TMUX_SESSION)"
tmux new-session -d -s "$TMUX_SESSION" -c "$DEMO_REPO" "$LF_BIN loop $WAVE"
say "one command, two processes: the listener boots, then spawns the resident"
say "the listener and resident both narrate into the same pane."
say "watch it live in another terminal:  tmux attach -r -t $TMUX_SESSION"
poll "endpoint published ($ENDPOINT_FILE)" 90 test -s "$ENDPOINT_FILE" || {
    warn "server never published its endpoint; last tmux output:"
    tmux capture-pane -p -t "$TMUX_SESSION" | tail -20
    exit 1
}
ADDR="$(cat "$ENDPOINT_FILE")"
say "endpoint: http://$ADDR"
pause

# ---------- act 1: health + journal ---------------------------------------

hr "act 1 · health and the journal spine"
say "health.loop_state is null until the resident attaches, then the loop's state:"
poll "resident attached (loop reported)" 90 sh -c \
    "curl -sf http://$ADDR/health | jq -e '.loop_state != null'" || true
curl -sf "http://$ADDR/health" | jq . | sed 's/^/  /'
say "journal (truth; the LISTENER holds the pen — the resident publishes"
say "turn deltas through the token-gated /resident door):"
poll "first journal rows" 30 test -s "$JOURNAL" || true
journal_types
say "look for: server_started (this boot) and thread_started (the vendor"
say "thread id — reported over the wire, the loop's first durable act)"
pause

if [[ $SMOKE -eq 1 ]]; then
    hr "smoke · skipping model acts 2-7 (each burns codex turns)"
else

# ---------- act 2: chat ----------------------------------------------------

hr "act 2 · send a message, watch the turn"
say 'POST /messages {"op":"message"} — queued; the loop answers at the next boundary'
curl -sf -X POST "http://$ADDR/messages" -H 'content-type: application/json' \
    -d '{"op":"message","text":"Introduce yourself in two sentences, then list what is on TODO.md."}' |
    jq -c '{state}' | sed 's/^/  /'
say "polling the thread until the turn finalizes (SSE is the live view:"
say "  curl -N http://$ADDR/events )"
poll "assistant turn finalized" 180 sh -c \
    "curl -sf http://$ADDR/conversation | jq -e '.turns[-1] | .role == \"assistant\" and .status != \"running\" and .status != \"pending\"'" || true
thread
pause

# ---------- act 3: steer mid-turn ------------------------------------------

hr "act 3 · steer a live turn"
say "start a longer turn, then redirect it mid-flight with {\"op\":\"steer\"}"
curl -sf -X POST "http://$ADDR/messages" -H 'content-type: application/json' \
    -d '{"op":"message","text":"Review TODO.md item by item: for each, describe what done looks like and estimate the work in detail."}' >/dev/null
sleep 3
curl -sf -X POST "http://$ADDR/messages" -H 'content-type: application/json' \
    -d '{"op":"steer","text":"Stop the detailed review - just name the single most valuable item and why, in one line."}' |
    jq -c '{state}' | sed 's/^/  /'
say "look for: the turn's output pivots; journal gains turn_steered naming the"
say "consumed message. If the first turn finished before the steer landed, the"
say "steer degrades to a queued message (documented) and answers next turn."
poll "turn finalized after steer" 180 sh -c \
    "curl -sf http://$ADDR/conversation | jq -e '.turns[-1] | .role == \"assistant\" and .status != \"running\" and .status != \"pending\"'" || true
journal_has turn_steered && ok "turn_steered journaled" || warn "no turn_steered — the steer likely arrived while idle (degraded to message)"
thread
pause

# ---------- act 4: interrupt ------------------------------------------------

hr "act 4 · interrupt — a partial turn is a value, not a crash"
curl -sf -X POST "http://$ADDR/messages" -H 'content-type: application/json' \
    -d '{"op":"message","text":"Enumerate 50 hypothetical improvements to this repo, in detail."}' >/dev/null
sleep 4
say 'POST {"op":"interrupt","text":""} — cooperative cancel, 10s force deadline'
curl -sf -X POST "http://$ADDR/messages" -H 'content-type: application/json' \
    -d '{"op":"interrupt","text":""}' | jq -c '{state}' | sed 's/^/  /'
poll "loop back to idle" 30 sh -c "[ \"\$(curl -sf http://$ADDR/health | jq -r .loop_state)\" = idle ]" || true
say "last turn (look for status=interrupted; if the turn beat the interrupt,"
say "an idle interrupt is a documented no-op):"
thread | tail -3
if [[ -z "$(new_codex_orphans)" ]]; then
    ok "no stray pass children after an interrupt (the child is killed)"
fi
pause

# ---------- act 5: dispatch a worker ----------------------------------------

hr "act 5 · the loop dispatches a worker (lf q worker run)"
say "asking the loop to delegate — orchestration lives in the prompt, loopflow is the toolset"
curl -sf -X POST "http://$ADDR/messages" -H 'content-type: application/json' \
    -d '{"op":"message","text":"Dispatch one worker via lf q worker run to make the next TODO.md improvement. Do not do the work inline. After dispatching, reply with the run id."}' >/dev/null
poll "run_observed journaled (loop pass + dispatch; model-dependent)" 300 journal_has run_observed || true
if journal_has run_observed; then
    jq -c 'select(.kind.type == "run_observed") | .kind' "$JOURNAL" | sed 's/^/  /'
    say "the worker is a real detached tmux session:"
    tmux list-sessions 2>/dev/null | grep -v "^$TMUX_SESSION" | sed 's/^/  /' || true
    say "and a real sibling worktree — <repo>.<wave>.<id> (three segments = wave worker):"
    ls -d "$DEMO_ROOT"/demorepo.$WAVE.* 2>/dev/null | sed 's/^/  /' || warn "worktree not visible yet"
else
    warn "no dispatch observed — read the loop's reply above and its tmux pane"
fi
pause

# ---------- act 6: worker reports + memory -----------------------------------

hr "act 6 · attributed reports and curated memory"
say "workers finish with 'lf chat <report>' — it lands in the thread with a"
say "from:\"worker\" byline and wakes the loop; watch for memory_updated when"
say "the loop curates what it learned (lf memory add)."
poll "a from-attributed worker report in the thread (workers take minutes)" 600 sh -c \
    "curl -sf http://$ADDR/conversation | jq -e '.turns[] | select(.from == \"worker\")'" || true
thread | tail -6
poll "run_completed journaled" 120 journal_has run_completed || true
if journal_has memory_updated; then
    ok "memory_updated journaled — the loop curated MEMORY.md unprompted:"
    curl -sf "http://$ADDR/memory" | jq -r .content | sed 's/^/  /'
else
    warn "no memory_updated yet — curation is the loop's judgment call, not a scripted step"
fi
pause

# ---------- act 7: restart, thread intact -------------------------------------

hr "act 7 · restart the server mid-conversation"
TURNS_BEFORE="$(curl -sf "http://$ADDR/health" | jq -r .turns)"
say "turns before restart: $TURNS_BEFORE — Ctrl-C the server, boot a new one"
tmux send-keys -t "$TMUX_SESSION" C-c
poll "endpoint removed on shutdown" 30 sh -c "! test -e '$ENDPOINT_FILE'" || true
tmux send-keys -t "$TMUX_SESSION" "$LF_BIN wave $WAVE" Enter
poll "endpoint re-published" 90 test -s "$ENDPOINT_FILE" || exit 1
ADDR="$(cat "$ENDPOINT_FILE")"
TURNS_AFTER="$(curl -sf "http://$ADDR/health" | jq -r .turns)"
if (( TURNS_AFTER >= TURNS_BEFORE )); then
    ok "thread intact after restart ($TURNS_AFTER turns; journal replay, no amnesia)"
else
    warn "turn count dropped: $TURNS_BEFORE -> $TURNS_AFTER"
fi
say "the vendor thread cold-starts (journaled as a fresh thread_started); the"
say "visible conversation survives because the journal is truth. The restarted"
say "listener spawned a fresh resident — the old one exited when its keeper died."
pause

fi  # SMOKE

# ---------- act 8: teardown ----------------------------------------------------

hr "act 8 · teardown — Ctrl-C, then verify nothing leaked"
tmux send-keys -t "$TMUX_SESSION" C-c
poll "endpoint removed" 30 sh -c "! test -e '$ENDPOINT_FILE'" || warn "stale .wave-endpoint left behind"
sleep 1
ORPHANS="$(new_codex_orphans)"
if [[ -z "$ORPHANS" ]]; then
    ok "no orphaned codex processes (the interrupt hook kills the app-server group)"
else
    warn "orphaned codex pids: $ORPHANS"
fi
if ! pgrep -f "lf serve $WAVE" >/dev/null 2>&1; then
    ok "no orphaned resident (the listener SIGTERMs its tenant on shutdown)"
else
    warn "Loop process still running: $(pgrep -f "lf serve $WAVE")"
fi
journal_types
say "demo repo kept for inspection: $DEMO_REPO"
say "worker tmux sessions/worktrees (if any) are yours to poke at, then delete:"
say "  rm -rf $DEMO_ROOT"
hr "done"
