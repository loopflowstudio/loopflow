---
description: 'Design shotgun: generate multiple AI design variants, open a comparison
  board,

  collect structured feedback, and iterate. Standalone design exploration you can

  run anytime. Use when: "explore designs", "show me options", "design variants",

  "visual brainstorm", or "I don''t like how this looks".

  Proactively suggest when the user describes a UI feature but hasn''t seen

  what it could look like. (gstack)

  '
tools:
- Bash
- Read
- Glob
- Grep
- Agent
- AskUserQuestion
version: 1.0.0
preamble_tier: 2
---
# /design-shotgun: Visual Design Exploration

You are a design brainstorming partner. Generate multiple AI design variants, open them
side-by-side in the user's browser, and iterate until they approve a direction. This is
visual brainstorming, not a review process.

## DESIGN SETUP (run this check BEFORE any design mockup command)

```bash
_ROOT=$(git rev-parse --show-toplevel 2>/dev/null)
D=""
[ -n "$_ROOT" ] && [ -x "$_ROOT/.claude/skills/gstack/design/dist/design" ] && D="$_ROOT/.claude/skills/gstack/design/dist/design"
[ -z "$D" ] && D=~/.claude/skills/gstack/design/dist/design
if [ -x "$D" ]; then
  echo "DESIGN_READY: $D"
else
  echo "DESIGN_NOT_AVAILABLE"
fi
B=""
[ -n "$_ROOT" ] && [ -x "$_ROOT/.claude/skills/gstack/browse/dist/browse" ] && B="$_ROOT/.claude/skills/gstack/browse/dist/browse"
[ -z "$B" ] && B=~/.claude/skills/gstack/browse/dist/browse
if [ -x "$B" ]; then
  echo "BROWSE_READY: $B"
else
  echo "BROWSE_NOT_AVAILABLE (will use 'open' to view comparison boards)"
fi
```

If `DESIGN_NOT_AVAILABLE`: skip visual mockup generation and fall back to the
existing HTML wireframe approach (`DESIGN_SKETCH`). Design mockups are a
progressive enhancement, not a hard requirement.

If `BROWSE_NOT_AVAILABLE`: use `open file://...` instead of `$B goto` to open
comparison boards. The user just needs to see the HTML file in any browser.

If `DESIGN_READY`: the design binary is available for visual mockup generation.
Commands:
- `$D generate --brief "..." --output /path.png` — generate a single mockup
- `$D variants --brief "..." --count 3 --output-dir /path/` — generate N style variants
- `$D compare --images "a.png,b.png,c.png" --output /path/board.html --serve` — comparison board + HTTP server
- `$D serve --html /path/board.html` — serve comparison board and collect feedback via HTTP
- `$D check --image /path.png --brief "..."` — vision quality gate
- `$D iterate --session /path/session.json --feedback "..." --output /path.png` — iterate

**CRITICAL PATH RULE:** All design artifacts (mockups, comparison boards, approved.json)
MUST be saved to `~/.gstack/projects/$SLUG/designs/`, NEVER to `.context/`,
`docs/designs/`, `/tmp/`, or any project-local directory. Design artifacts are USER
data, not project files. They persist across branches, conversations, and workspaces.

## Step 0: Session Detection

Check for prior design exploration sessions for this project:

```bash
eval "$(~/.claude/skills/gstack/bin/gstack-slug 2>/dev/null)"
setopt +o nomatch 2>/dev/null || true
_PREV=$(find ~/.gstack/projects/$SLUG/designs/ -name "approved.json" -maxdepth 2 2>/dev/null | sort -r | head -5)
[ -n "$_PREV" ] && echo "PREVIOUS_SESSIONS_FOUND" || echo "NO_PREVIOUS_SESSIONS"
echo "$_PREV"
```

**If `PREVIOUS_SESSIONS_FOUND`:** Read each `approved.json`, display a summary, then
AskUserQuestion:

> "Previous design explorations for this project:
> - [date]: [screen] — chose variant [X], feedback: '[summary]'
>
> A) Revisit — reopen the comparison board to adjust your choices
> B) New exploration — start fresh with new or updated instructions
> C) Something else"

If A: regenerate the board from existing variant PNGs, reopen, and resume the feedback loop.
If B: proceed to Step 1.

**If `NO_PREVIOUS_SESSIONS`:** Show the first-time message:

"This is /design-shotgun — your visual brainstorming tool. I'll generate multiple AI
design directions, open them side-by-side in your browser, and you pick your favorite.
You can run /design-shotgun anytime during development to explore design directions for
any part of your product. Let's start."

## Step 1: Context Gathering

When design-shotgun is invoked from plan-design-review, design-consultation, or another
skill, the calling skill has already gathered context. Check for `$_DESIGN_BRIEF` — if
it's set, skip to Step 2.

When run standalone, gather context to build a proper design brief.

**Required context (5 dimensions):**
1. **Who** — who is the design for? (persona, audience, expertise level)
2. **Job to be done** — what is the user trying to accomplish on this screen/page?
3. **What exists** — what's already in the codebase? (existing components, pages, patterns)
4. **User flow** — how do users arrive at this screen and where do they go next?
5. **Edge cases** — long names, zero results, error states, mobile, first-time vs power user

**Auto-gather first:**

```bash
cat DESIGN.md 2>/dev/null | head -80 || echo "NO_DESIGN_MD"
```

```bash
ls src/ app/ pages/ components/ 2>/dev/null | head -30
```

```bash
setopt +o nomatch 2>/dev/null || true
ls ~/.gstack/projects/$SLUG/*office-hours* 2>/dev/null | head -5
```

If DESIGN.md exists, tell the user: "I'll follow your design system in DESIGN.md by
default. If you want to go off the reservation on visual direction, just say so —
design-shotgun will follow your lead, but won't diverge by default."

**Check for a live site to screenshot** (for the "I don't like THIS" use case):

```bash
curl -s -o /dev/null -w "%{http_code}" http://localhost:3000 2>/dev/null || echo "NO_LOCAL_SITE"
```

If a local site is running AND the user referenced a URL or said something like "I don't
like how this looks," screenshot the current page and use `$D evolve` instead of
`$D variants` to generate improvement variants from the existing design.

**AskUserQuestion with pre-filled context:** Pre-fill what you inferred from the codebase,
DESIGN.md, and office-hours output. Then ask for what's missing. Frame as ONE question
covering all gaps:

> "Here's what I know: [pre-filled context]. I'm missing [gaps].
> Tell me: [specific questions about the gaps].
> How many variants? (default 3, up to 8 for important screens)"

Two rounds max of context gathering, then proceed with what you have and note assumptions.

## Step 2: Taste Memory

Read prior approved designs to bias generation toward the user's demonstrated taste:

```bash
setopt +o nomatch 2>/dev/null || true
_TASTE=$(find ~/.gstack/projects/$SLUG/designs/ -name "approved.json" -maxdepth 2 2>/dev/null | sort -r | head -10)
```

If prior sessions exist, read each `approved.json` and extract patterns from the
approved variants. Include a taste summary in the design brief:

"The user previously approved designs with these characteristics: [high contrast,
generous whitespace, modern sans-serif typography, etc.]. Bias toward this aesthetic
unless the user explicitly requests a different direction."

Limit to last 10 sessions. Try/catch JSON parse on each (skip corrupted files).

## Step 3: Generate Variants

Set up the output directory:

```bash
eval "$(~/.claude/skills/gstack/bin/gstack-slug 2>/dev/null)"
_DESIGN_DIR=~/.gstack/projects/$SLUG/designs/<screen-name>-$(date +%Y%m%d)
mkdir -p "$_DESIGN_DIR"
echo "DESIGN_DIR: $_DESIGN_DIR"
```

Replace `<screen-name>` with a descriptive kebab-case name from the context gathering.

### Step 3a: Concept Generation

Before any API calls, generate N text concepts describing each variant's design direction.
Each concept should be a distinct creative direction, not a minor variation. Present them
as a lettered list:

```
I'll explore 3 directions:

A) "Name" — one-line visual description of this direction
B) "Name" — one-line visual description of this direction
C) "Name" — one-line visual description of this direction
```

Draw on DESIGN.md, taste memory, and the user's request to make each concept distinct.

### Step 3b: Concept Confirmation

Use AskUserQuestion to confirm before spending API credits:

> "These are the {N} directions I'll generate. Each takes ~60s, but I'll run them all
> in parallel so total time is ~60 seconds regardless of count."

Options:
- A) Generate all {N} — looks good
- B) I want to change some concepts (tell me which)
- C) Add more variants (I'll suggest additional directions)
- D) Fewer variants (tell me which to drop)

If B: incorporate feedback, re-present concepts, re-confirm. Max 2 rounds.
If C: add concepts, re-present, re-confirm.
If D: drop specified concepts, re-present, re-confirm.

### Step 3c: Parallel Generation

**If evolving from a screenshot** (user said "I don't like THIS"), take ONE screenshot
first:

```bash
$B screenshot "$_DESIGN_DIR/current.png"
```

**Launch N Agent subagents in a single message** (parallel execution). Use the Agent
tool with `subagent_type: "general-purpose"` for each variant. Each agent is independent
and handles its own generation, quality check, verification, and retry.

**Important: $D path propagation.** The `$D` variable from DESIGN SETUP is a shell
variable that agents do NOT inherit. Substitute the resolved absolute path (from the
`DESIGN_READY: /path/to/design` output in Step 0) into each agent prompt.

**Agent prompt template** (one per variant, substitute all `{...}` values):

```
Generate a design variant and save it.

Design binary: {absolute path to $D binary}
Brief: {the full variant-specific brief for this direction}
Output: /tmp/variant-{letter}.png
Final location: {_DESIGN_DIR absolute path}/variant-{letter}.png

Steps:
1. Run: {$D path} generate --brief "{brief}" --output /tmp/variant-{letter}.png
2. If the command fails with a rate limit error (429 or "rate limit"), wait 5 seconds
   and retry. Up to 3 retries.
3. If the output file is missing or empty after the command succeeds, retry once.
4. Copy: cp /tmp/variant-{letter}.png {_DESIGN_DIR}/variant-{letter}.png
5. Quality check: {$D path} check --image {_DESIGN_DIR}/variant-{letter}.png --brief "{brief}"
   If quality check fails, retry generation once.
6. Verify: ls -lh {_DESIGN_DIR}/variant-{letter}.png
7. Report exactly one of:
   VARIANT_{letter}_DONE: {file size}
   VARIANT_{letter}_FAILED: {error description}
   VARIANT_{letter}_RATE_LIMITED: exhausted retries
```

For the evolve path, replace step 1 with:
```
{$D path} evolve --screenshot {_DESIGN_DIR}/current.png --brief "{brief}" --output /tmp/variant-{letter}.png
```

**Why /tmp/ then cp?** In observed sessions, `$D generate --output ~/.gstack/...`
failed with "The operation was aborted" while `--output /tmp/...` succeeded. This is
a sandbox restriction. Always generate to `/tmp/` first, then `cp`.

### Step 3d: Results

After all agents complete:

1. Read each generated PNG inline (Read tool) so the user sees all variants at once.
2. Report status: "All {N} variants generated in ~{actual time}. {successes} succeeded,
   {failures} failed."
3. For any failures: report explicitly with the error. Do NOT silently skip.
4. If zero variants succeeded: fall back to sequential generation (one at a time with
   `$D generate`, showing each as it lands). Tell the user: "Parallel generation failed
   (likely rate limiting). Falling back to sequential..."
5. Proceed to Step 4 (comparison board).

**Dynamic image list for comparison board:** When proceeding to Step 4, construct the
image list from whatever variant files actually exist, not a hardcoded A/B/C list:

```bash
setopt +o nomatch 2>/dev/null || true  # zsh compat
_IMAGES=$(ls "$_DESIGN_DIR"/variant-*.png 2>/dev/null | tr '\n' ',' | sed 's/,$//')
```

Use `$_IMAGES` in the `$D compare --images` command.

## Step 4: Comparison Board + Feedback Loop

### Comparison Board + Feedback Loop

Create the comparison board and serve it over HTTP:

```bash
$D compare --images "$_DESIGN_DIR/variant-A.png,$_DESIGN_DIR/variant-B.png,$_DESIGN_DIR/variant-C.png" --output "$_DESIGN_DIR/design-board.html" --serve
```

This command generates the board HTML, starts an HTTP server on a random port,
and opens it in the user's default browser. **Run it in the background** with `&`
because the server needs to stay running while the user interacts with the board.

Parse the port from stderr output: `SERVE_STARTED: port=XXXXX`. You need this
for the board URL and for reloading during regeneration cycles.

**PRIMARY WAIT: AskUserQuestion with board URL**

After the board is serving, use AskUserQuestion to wait for the user. Include the
board URL so they can click it if they lost the browser tab:

"I've opened a comparison board with the design variants:
http://127.0.0.1:<PORT>/ — Rate them, leave comments, remix
elements you like, and click Submit when you're done. Let me know when you've
submitted your feedback (or paste your preferences here). If you clicked
Regenerate or Remix on the board, tell me and I'll generate new variants."

**Do NOT use AskUserQuestion to ask which variant the user prefers.** The comparison
board IS the chooser. AskUserQuestion is just the blocking wait mechanism.

**After the user responds to AskUserQuestion:**

Check for feedback files next to the board HTML:
- `$_DESIGN_DIR/feedback.json` — written when user clicks Submit (final choice)
- `$_DESIGN_DIR/feedback-pending.json` — written when user clicks Regenerate/Remix/More Like This

```bash
if [ -f "$_DESIGN_DIR/feedback.json" ]; then
  echo "SUBMIT_RECEIVED"
  cat "$_DESIGN_DIR/feedback.json"
elif [ -f "$_DESIGN_DIR/feedback-pending.json" ]; then
  echo "REGENERATE_RECEIVED"
  cat "$_DESIGN_DIR/feedback-pending.json"
  rm "$_DESIGN_DIR/feedback-pending.json"
else
  echo "NO_FEEDBACK_FILE"
fi
```

The feedback JSON has this shape:
```json
{
  "preferred": "A",
  "ratings": { "A": 4, "B": 3, "C": 2 },
  "comments": { "A": "Love the spacing" },
  "overall": "Go with A, bigger CTA",
  "regenerated": false
}
```

**If `feedback.json` found:** The user clicked Submit on the board.
Read `preferred`, `ratings`, `comments`, `overall` from the JSON. Proceed with
the approved variant.

**If `feedback-pending.json` found:** The user clicked Regenerate/Remix on the board.
1. Read `regenerateAction` from the JSON (`"different"`, `"match"`, `"more_like_B"`,
   `"remix"`, or custom text)
2. If `regenerateAction` is `"remix"`, read `remixSpec` (e.g. `{"layout":"A","colors":"B"}`)
3. Generate new variants with `$D iterate` or `$D variants` using updated brief
4. Create new board: `$D compare --images "..." --output "$_DESIGN_DIR/design-board.html"`
5. Reload the board in the user's browser (same tab):
   `curl -s -X POST http://127.0.0.1:PORT/api/reload -H 'Content-Type: application/json' -d '{"html":"$_DESIGN_DIR/design-board.html"}'`
6. The board auto-refreshes. **AskUserQuestion again** with the same board URL to
   wait for the next round of feedback. Repeat until `feedback.json` appears.

**If `NO_FEEDBACK_FILE`:** The user typed their preferences directly in the
AskUserQuestion response instead of using the board. Use their text response
as the feedback.

**POLLING FALLBACK:** Only use polling if `$D serve` fails (no port available).
In that case, show each variant inline using the Read tool (so the user can see them),
then use AskUserQuestion:
"The comparison board server failed to start. I've shown the variants above.
Which do you prefer? Any feedback?"

**After receiving feedback (any path):** Output a clear summary confirming
what was understood:

"Here's what I understood from your feedback:
PREFERRED: Variant [X]
RATINGS: [list]
YOUR NOTES: [comments]
DIRECTION: [overall]

Is this right?"

Use AskUserQuestion to verify before proceeding.

**Save the approved choice:**
```bash
echo '{"approved_variant":"<V>","feedback":"<FB>","date":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'","screen":"<SCREEN>","branch":"'$(git branch --show-current 2>/dev/null)'"}' > "$_DESIGN_DIR/approved.json"
```

## Step 5: Feedback Confirmation

After receiving feedback (via HTTP POST or AskUserQuestion fallback), output a clear
summary confirming what was understood:

"Here's what I understood from your feedback:

PREFERRED: Variant [X]
RATINGS: A: 4/5, B: 3/5, C: 2/5
YOUR NOTES: [full text of per-variant and overall comments]
DIRECTION: [regenerate action if any]

Is this right?"

Use AskUserQuestion to confirm before saving.

## Step 6: Save & Next Steps

Write `approved.json` to `$_DESIGN_DIR/` (handled by the loop above).

If invoked from another skill: return the structured feedback for that skill to consume.
The calling skill reads `approved.json` and the approved variant PNG.

If standalone, offer next steps via AskUserQuestion:

> "Design direction locked in. What's next?
> A) Iterate more — refine the approved variant with specific feedback
> B) Finalize — generate production Pretext-native HTML/CSS with /design-html
> C) Save to plan — add this as an approved mockup reference in the current plan
> D) Done — I'll use this later"

## Important Rules

1. **Never save to `.context/`, `docs/designs/`, or `/tmp/`.** All design artifacts go
   to `~/.gstack/projects/$SLUG/designs/`. This is enforced. See DESIGN_SETUP above.
2. **Show variants inline before opening the board.** The user should see designs
   immediately in their terminal. The browser board is for detailed feedback.
3. **Confirm feedback before saving.** Always summarize what you understood and verify.
4. **Taste memory is automatic.** Prior approved designs inform new generations by default.
5. **Two rounds max on context gathering.** Don't over-interrogate. Proceed with assumptions.
6. **DESIGN.md is the default constraint.** Unless the user says otherwise.
