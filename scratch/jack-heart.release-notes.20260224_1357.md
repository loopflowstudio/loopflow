# LLM-Generated Release Notes

`lf ops release v0.9.1` — one command generates user-centric release notes, creates an auto-landing PR, and triggers the full release pipeline.

## What to build

1. Gathers merged PRs since last tag via `gh`
2. LLM translates PR list into themed, user-centric notes
3. Writes `RELEASE_NOTES.md`
4. Creates auto-merging PR on `release/v0.9.1` branch
5. CI detects merge, creates tag → existing release.yml fires
6. GitHub release body reads from `RELEASE_NOTES.md`

Version stays tag-derived. No Cargo.toml bump in the PR.

## Data structures

```rust
// ops/release.rs

/// A merged PR with enough context for release notes.
struct MergedPr {
    number: u32,
    title: String,
    body: String,
}

pub fn release(repo: &Path, version: &str, progress: &impl Progress) -> OpsResult<String> {
    let prev_tag = latest_tag(repo)?;
    let prs = merged_prs_since(repo, &prev_tag)?;
    let notes = generate_release_notes(repo, &prs, version)?;
    write_release_notes(repo, &notes, version)?;
    create_release_pr(repo, version, progress)
}
```

## Key functions

`latest_tag(repo)` — `git describe --tags --abbrev=0` to find the previous release.

`merged_prs_since(repo, tag)` — `gh pr list --state merged --base main --search "merged:>={date}" --json number,title,body`. Gets the tag date via `git log -1 --format=%aI {tag}`.

`generate_release_notes(repo, prs, version)` — Builds prompt from `get_builtin_ops_prompt("release_notes")` + formatted PR list. Calls `launch_agent()` in batch mode. Returns raw markdown string (no JSON parsing — output goes straight to file).

`write_release_notes(repo, notes, version)` — Writes `RELEASE_NOTES.md` at repo root. Prepends `# v{version}\n\n` if the LLM didn't include it.

`create_release_pr(repo, version, progress)` — Creates branch `release/v{version}`, commits RELEASE_NOTES.md, pushes, runs `gh pr create --fill --label release` then `gh pr merge --auto --squash`.

## Files to change

| File | Change |
|------|--------|
| `rust/loopflow/src/ops/release.rs` | **New.** Core logic (~150 LOC) |
| `rust/loopflow/src/ops/mod.rs` | Add `mod release` + re-export |
| `rust/loopflow/src/lf/mod.rs` | Add `Release { version: String }` to `OpsCommand` |
| `rust/loopflow/src/lf/commands/ops/mod.rs` | Add handler: match `Release` → call `release()` |
| `rust/loopflow/src/engine/builtins/ops/release_notes.md` | Update: output markdown not JSON, add theming guidance |
| `.github/workflows/auto-tag.yml` | **New.** Detect RELEASE_NOTES.md merge, create + push tag |
| `.github/workflows/release.yml` | One line: `body_path: RELEASE_NOTES.md` replaces `generate_release_notes: true` |

## Prompt update

The existing `release_notes.md` prompt is good. Changes:

- Output is raw markdown (not JSON) — it writes directly to the file
- First line must be `# v{version}` (CI parses this for tagging)
- Add theming guidance: group changes by what users care about (new capabilities, improvements, security, infrastructure) not by codebase area
- Emphasize: "skip internal refactors unless they affect what users experience"

## CI: auto-tag workflow

```yaml
# .github/workflows/auto-tag.yml
name: Auto Tag Release
on:
  push:
    branches: [main]
    paths: [RELEASE_NOTES.md]

jobs:
  tag:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
      - name: Extract version and create tag
        run: |
          version=$(head -1 RELEASE_NOTES.md | sed -n 's/^# v\(.*\)/v\1/p')
          if [ -z "$version" ]; then
            echo "No version found in RELEASE_NOTES.md header"
            exit 0
          fi
          if git ls-remote --tags origin "$version" | grep -q .; then
            echo "Tag $version already exists"
            exit 0
          fi
          git tag "$version"
          git push origin "$version"
```

## Constraints

- **Version is explicit.** User passes it: `lf ops release v0.9.1`. No auto-bumping.
- **RELEASE_NOTES.md is the single source.** CI reads it, GitHub release body comes from it, repo has it.
- **Auto-merge needs repo setting.** `gh pr merge --auto` requires "allow auto-merge" enabled.

## Done when

```bash
lf ops release v0.9.1
# → RELEASE_NOTES.md written with LLM-generated notes
# → PR created on release/v0.9.1, auto-merge enabled
# → after merge: tag v0.9.1 pushed by CI
# → GitHub release body matches RELEASE_NOTES.md
```
