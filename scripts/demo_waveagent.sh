#!/usr/bin/env bash
# Demo: the wave-agent model shipped so far — a wave is two files (GOAL.md +
# MEMORY.md), its memory is injected into the loop's context, and its live
# sessions are watchable/enterable via tmux.
#
# Run from the repo root:  bash scripts/demo_waveagent.sh
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

hr() { printf '\n\033[1;35m%s\033[0m\n' "── $* ──"; }

hr "1. A wave is two files on disk"
echo "wave/goals/ holds exactly the authored surface + curated memory:"
ls -1 wave/goals/ | grep -E '^(GOAL|MEMORY)\.md$' | sed 's/^/  /'
echo
echo "GOAL.md — intent (frontmatter + loop prompt):"
sed -n '1,12p' wave/goals/GOAL.md | sed 's/^/  /'
echo
echo "MEMORY.md — the wave's curated, continuity memory:"
sed -n '1,8p' wave/goals/MEMORY.md | sed 's/^/  /'

hr "2. Memory is injected into the wave's assembled context"
echo "Rendering the loop prompt for the 'goals' wave (lf-prompt --wave goals)…"
tmp="$(mktemp)"
cargo run -q -p loopflow --bin lf-prompt -- --repo . --wave goals >"$tmp" 2>/dev/null
echo "The rendered prompt carries MEMORY.md verbatim — proof it reaches every"
echo "context the wave runs (agent loop + dispatched subagents):"
grep -nE 'goals wave memory|Steers Loopflow' "$tmp" | head -3 | sed 's/^/  /'
rm -f "$tmp"

hr "3. Live sessions are watchable and enterable"
echo "tmux is the session cockpit:"
echo "  tmux ls                 # list live agent + dispatch sessions"
echo "  tmux attach -t <name>   # drop into one to answer an interactive step"
echo
echo "(Requires active dispatched sessions; dispatch with 'lf q worker run'.)"

hr "Done"
echo "Two files define a wave; its memory rides into every prompt; its sessions are"
echo "watchable and steerable. Next: dispatch-through-lfd makes each subagent its own"
echo "attachable session."
