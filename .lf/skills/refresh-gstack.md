---
requires: inside the loopflow repo
produces: updated files under rust/loopflow/src/engine/builtins/gstack/
---
Pull the latest gstack workstyle from garrytan/gstack and regenerate the
built-in gstack steps, flows, and direction. This step only updates the files.
Committing, opening a PR, or deploying is out of scope.

## Goal

Keep loopflow's built-in `gstack/*` catalog in sync with upstream. When the
step finishes, the working tree has the latest converted steps, flows, and
direction; nothing has been committed.

## Guardrails

- Run from inside the loopflow repo (the one that ships `lf`). Abort if the
  current directory isn't a loopflow checkout — namespaced builtins live only
  in this tree.
- Do not modify anything outside `rust/loopflow/src/engine/builtins/gstack/`
  and `rust/loopflow/src/engine/builtins/directions/gstack.md`.
- Do not create commits, tags, or PRs. Leave the diff for a human (or a
  follow-up `ship`/`deploy` flow) to review.

## Workflow

1. **Locate the loopflow root.**
   ```bash
   git rev-parse --show-toplevel
   ls rust/loopflow/src/engine/builtins/gstack >/dev/null
   ```
   If the `builtins/gstack/` directory isn't there, you're in the wrong repo —
   stop.

2. **Refresh the upstream cache.**
   ```bash
   CACHE="$HOME/.lf/cache/gstack"
   if [ -d "$CACHE/.git" ]; then
     git -C "$CACHE" fetch origin main
     git -C "$CACHE" reset --hard origin/main
   else
     mkdir -p "$(dirname "$CACHE")"
     git clone https://github.com/garrytan/gstack.git "$CACHE"
   fi
   UPSTREAM_HEAD="$(git -C "$CACHE" rev-parse HEAD)"
   ```

3. **Run the converter.**
   ```bash
   uv run python -m loopflow.workstyle.convert \
     "$CACHE" \
     rust/loopflow/src/engine/builtins/gstack/step \
     --direction-output rust/loopflow/src/engine/builtins/directions/gstack.md
   ```
   The converter reads each `SKILL.md` in the upstream repo, rewrites it into
   loopflow step format, and writes `workstyle.yaml` alongside the steps.

4. **Inspect the diff.**
   ```bash
   git status --short rust/loopflow/src/engine/builtins/gstack
   git diff --stat rust/loopflow/src/engine/builtins/gstack
   ```
   Scan the output for:
   - Added or removed step files (upstream renames, additions, drops)
   - Flow-file churn in `builtins/gstack/flow/` — if upstream changed sprint
     composition, the YAML under `flow/` may need a manual update (the
     converter writes steps but not flows).
   - Direction drift in `directions/gstack.md`.

5. **Build and test.**
   ```bash
   cargo build -p loopflow --bin lf
   cargo test -p loopflow --test discovery_tests \
     every_builtin_step_is_categorized_and_discoverable
   ```
   The categorization test ensures every new step file is picked up. If it
   fails with "builtin step X has no description", the upstream skill is
   missing a `description:` in its frontmatter — flag it in the summary.

6. **Summarize.**
   Report:
   - Upstream commit synced to
   - Added / changed / removed step names
   - Any flow-file changes needed (manual follow-up)
   - Build and test status

   Stop there. A human reviews the diff and drives any commit / PR /
   release step separately.

## What not to do

- Don't touch `.lf/steps/gstack/` — gstack is a built-in now; that path is
  the old sync target and is gone.
- Don't modify flow YAML under `builtins/gstack/flow/` unless upstream made a
  compositional change that the converter couldn't express (note it in the
  summary and let the human decide).
- Don't run `git commit`, `git push`, `gh pr`, `lf ship`, or `lf op land`.
  This step updates the tree; deploy is a separate concern.
