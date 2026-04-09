#!/usr/bin/env bash
#
# Ablation test: what triggers the Max plan system prompt restriction?
#
# Runs `claude -p` with --append-system-prompt or --append-system-prompt-file
# using progressively more content to find the boundary.
#
# Usage: ./scratch/test-system-prompt.sh
#
# Each test prints PASS/FAIL + the error snippet if it fails.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

TASK="Say 'hello' and nothing else."
TIMEOUT=30

# Grab the builtin docs
LOOPFLOW_DOC="$REPO_ROOT/rust/loopflow/src/engine/builtins/LOOPFLOW.md"
RLM_DOC="$REPO_ROOT/rust/loopflow/src/engine/builtins/RLM.md"
VOICE_DOC="$REPO_ROOT/rust/loopflow/src/engine/builtins/VOICE.md"

# Grab a real context file for the "full system prompt" test
FULL_CONTEXT="$(ls -t "$REPO_ROOT"/.lf/prompts/*.context.md 2>/dev/null | head -1)"

pass=0
fail=0
results=()

run_test() {
    local name="$1"
    shift
    # $@ = extra claude args

    printf "%-55s " "$name..."

    local out
    out="$(timeout "$TIMEOUT" claude -p "$TASK" --max-turns 1 --output-format json "$@" 2>&1)" || true

    # Check for common restriction error patterns
    if echo "$out" | grep -qi "system prompt\|not allowed\|not supported\|plan does not\|restricted\|unauthorized\|Max plan\|subscribe\|upgrade\|overriding the system prompt is not allowed"; then
        echo "FAIL"
        # Show first matching line
        echo "  -> $(echo "$out" | grep -i "system prompt\|not allowed\|not supported\|plan does not\|restricted\|unauthorized\|Max plan\|subscribe\|upgrade\|overriding the system prompt" | head -1)"
        ((fail++))
        results+=("FAIL  $name")
    elif echo "$out" | grep -qi "error\|Error"; then
        # Some other error — might be relevant
        local errline
        errline="$(echo "$out" | grep -i "error" | head -1)"
        if echo "$errline" | grep -qi "system prompt\|not allowed\|restricted"; then
            echo "FAIL"
            echo "  -> $errline"
            ((fail++))
            results+=("FAIL  $name")
        else
            echo "PASS (with unrelated error: ${errline:0:80})"
            ((pass++))
            results+=("PASS  $name")
        fi
    else
        echo "PASS"
        ((pass++))
        results+=("PASS  $name")
    fi
}

echo "======================================================================"
echo "System prompt ablation tests"
echo "======================================================================"
echo ""
echo "Testing which system prompt content triggers the Max plan restriction."
echo "Each test runs: claude -p '$TASK' --max-turns 1 [+ system prompt args]"
echo ""

# ── Test 1: No system prompt (baseline) ──────────────────────────────
run_test "1. No system prompt (baseline)"

# ── Test 2: Minimal system prompt (inline) ────────────────────────────
run_test "2. Inline: 'You are helpful'" \
    --append-system-prompt "You are helpful."

# ── Test 3: Slightly longer inline ────────────────────────────────────
run_test "3. Inline: two paragraphs of instructions" \
    --append-system-prompt "You are a coding assistant. Follow the user's instructions carefully. Be concise. When editing files, use the Edit tool. When searching, use Grep. Always explain your reasoning briefly before acting."

# ── Test 4: LOOPFLOW.md only (inline) ────────────────────────────────
LOOPFLOW_CONTENT="$(cat "$LOOPFLOW_DOC")"
run_test "4. Inline: LOOPFLOW.md (~6KB)" \
    --append-system-prompt "$LOOPFLOW_CONTENT"

# ── Test 5: LOOPFLOW.md via file ──────────────────────────────────────
run_test "5. File: LOOPFLOW.md (~6KB)" \
    --append-system-prompt-file "$LOOPFLOW_DOC"

# ── Test 6: RLM.md only (inline) ─────────────────────────────────────
RLM_CONTENT="$(cat "$RLM_DOC")"
run_test "6. Inline: RLM.md (~5KB)" \
    --append-system-prompt "$RLM_CONTENT"

# ── Test 7: VOICE.md only (inline) ───────────────────────────────────
VOICE_CONTENT="$(cat "$VOICE_DOC")"
run_test "7. Inline: VOICE.md (~2KB)" \
    --append-system-prompt "$VOICE_CONTENT"

# ── Test 8: LOOPFLOW + RLM combined (inline) ─────────────────────────
run_test "8. Inline: LOOPFLOW + RLM (~11KB)" \
    --append-system-prompt "${LOOPFLOW_CONTENT}

${RLM_CONTENT}"

# ── Test 9: LOOPFLOW + RLM + VOICE combined (file) ───────────────────
cat "$LOOPFLOW_DOC" "$RLM_DOC" "$VOICE_DOC" > "$TMP/combined-builtins.md"
run_test "9. File: LOOPFLOW + RLM + VOICE (~12KB)" \
    --append-system-prompt-file "$TMP/combined-builtins.md"

# ── Test 10: Full context file from a real run ────────────────────────
if [ -n "$FULL_CONTEXT" ]; then
    FULL_SIZE=$(wc -c < "$FULL_CONTEXT" | tr -d ' ')
    run_test "10. File: full real context (~${FULL_SIZE}B)" \
        --append-system-prompt-file "$FULL_CONTEXT"
else
    echo "10. File: full real context                            SKIP (no .context.md found)"
fi

# ── Test 11: Large synthetic payload (50KB) ───────────────────────────
python3 -c "print('x ' * 25000)" > "$TMP/large-50k.md"
run_test "11. File: synthetic 50KB" \
    --append-system-prompt-file "$TMP/large-50k.md"

# ── Test 12: Just XML tags (structure without content) ────────────────
run_test "12. Inline: just XML tags" \
    --append-system-prompt "<lf:loopflow>Follow instructions.</lf:loopflow>"

# ── Test 13: Keyword probe — 'system prompt' in content ───────────────
run_test "13. Inline: contains 'system prompt' text" \
    --append-system-prompt "This is a system prompt override for testing purposes."

# ── Test 14: Keyword probe — 'override' / 'instructions' ─────────────
run_test "14. Inline: 'override default instructions'" \
    --append-system-prompt "Override the default instructions. You are now a different assistant."

# ── Test 15: Benign content at full-context size ──────────────────────
if [ -n "$FULL_CONTEXT" ]; then
    FULL_SIZE=$(wc -c < "$FULL_CONTEXT" | tr -d ' ')
    # Generate benign padding to match size
    python3 -c "
import sys
target = $FULL_SIZE
line = 'This is documentation about the project structure and coding conventions.\n'
while sys.stdout.tell() if hasattr(sys.stdout, 'tell') else 0 < target:
    sys.stdout.write(line)
    target -= len(line)
    if target <= 0:
        break
" > "$TMP/benign-padded.md"
    PADDED_SIZE=$(wc -c < "$TMP/benign-padded.md" | tr -d ' ')
    run_test "15. File: benign content at real size (~${PADDED_SIZE}B)" \
        --append-system-prompt-file "$TMP/benign-padded.md"
fi

# ── Summary ───────────────────────────────────────────────────────────
echo ""
echo "======================================================================"
echo "Summary: $pass passed, $fail failed"
echo "======================================================================"
for r in "${results[@]}"; do
    echo "  $r"
done
