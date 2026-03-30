from __future__ import annotations

import subprocess
from pathlib import Path

import yaml

from loopflow.workstyle.convert import convert_gstack_repo, extract_openclaw_direction


def test_convert_gstack_repo_writes_steps_manifest_and_direction(tmp_path: Path) -> None:
    source_repo = tmp_path / "gstack"
    source_repo.mkdir()
    (source_repo / "SKILL.md").write_text(
        """---
name: gstack
version: 1.0.0
description: Root skill.
allowed-tools:
  - Read
---
## Voice

Short root voice.

# /gstack — Root

Root instructions.
""",
        encoding="utf-8",
    )

    (source_repo / "office-hours").mkdir()
    (source_repo / "office-hours" / "SKILL.md").write_text(
        """---
name: office-hours
preamble-tier: 3
version: 2.0.0
description: Brainstorm and write a design doc.
allowed-tools:
  - Bash
  - Read
benefits-from: []
---
<!-- AUTO-GENERATED from SKILL.md.tmpl — do not edit directly -->

## Preamble (run first)

```bash
echo setup
```

## Voice

You are GStack.

Builders ship.

# /office-hours — Office Hours

```bash
mkdir -p ~/.gstack/analytics
echo '{"skill":"office-hours","ts":"2026-01-01T00:00:00Z"}'  >> ~/.gstack/analytics/skill-usage.jsonl 2>/dev/null || true
```

Do the work.

3. Append metrics:
```bash
mkdir -p ~/.gstack/analytics
echo '{"skill":"office-hours","ts":"2026-01-01T00:00:00Z","iterations":3}' >> ~/.gstack/analytics/spec-review.jsonl 2>/dev/null || true
```
Replace ITERATIONS, FOUND, FIXED, REMAINING, SCORE with actual values from the review.

## Voice & Tone

This should not survive conversion.
""",
        encoding="utf-8",
    )

    (source_repo / "plan-ceo-review").mkdir()
    (source_repo / "plan-ceo-review" / "SKILL.md").write_text(
        """---
name: plan-ceo-review
preamble-tier: 3
version: 1.0.0
description: Review scope and strategy.
allowed-tools:
  - Read
benefits-from:
  - office-hours
---
## Preamble (run first)

Ignored.

## Voice

Short voice.

# /plan-ceo-review — CEO Review

Focus on the wedge.

## Review Log

Persist review metadata.
""",
        encoding="utf-8",
    )

    (source_repo / "browse").mkdir()
    (source_repo / "browse" / "SKILL.md").write_text(
        """---
name: browse
version: 1.0.0
description: Browser-driven verification.
allowed-tools:
  - Bash
---
# /browse — Browser workflow

Keep browser setup instructions.

## SETUP

Build the browser tool once.
""",
        encoding="utf-8",
    )

    (source_repo / "retro").mkdir()
    (source_repo / "retro" / "SKILL.md").write_text(
        """---
name: retro
version: 1.0.0
description: Weekly retrospective.
allowed-tools:
  - Bash
---
# /retro — Weekly Engineering Retrospective

# 12. gstack skill usage telemetry (if available)
cat ~/.gstack/analytics/skill-usage.jsonl 2>/dev/null || true

**Skill Usage (if analytics exist):** Read `~/.gstack/analytics/skill-usage.jsonl` if it exists. Filter entries within the retro time window by `ts` field. Separate skill activations from hook fires.

```
| Skill Usage | /ship(12) /qa(8) |
```

If the JSONL file doesn't exist or has no entries in the window, skip the Skill Usage row.

**Eureka Moments (if logged):** Read `~/.gstack/analytics/eureka.jsonl` if it exists. Filter entries within the retro time window by `ts` field.

```
| Eureka Moments | 2 this period |
```

If the JSONL file doesn't exist or has no entries in the window, skip the Eureka Moments row.

### Step 3: Commit Time Distribution

Keep the actual retro.
""",
        encoding="utf-8",
    )

    output_dir = tmp_path / "out"
    direction_output = tmp_path / "gstack.md"
    _init_git_repo(source_repo)

    manifest = convert_gstack_repo(
        source_repo,
        output_dir,
        source_repo_name="garrytan/gstack",
        source_ref="main",
        direction_output=direction_output,
    )

    assert manifest.name == "gstack"
    assert manifest.source_repo == "garrytan/gstack"
    assert manifest.source_ref == "main"
    assert manifest.step_prefix == "gstack"
    assert manifest.steps == ["gstack", "browse", "office-hours", "ceo-review", "retro"]

    workstyle = yaml.safe_load(output_dir.joinpath("workstyle.yaml").read_text(encoding="utf-8"))
    assert workstyle["prefix"] == "gstack"
    assert workstyle["source"]["repo"] == "garrytan/gstack"
    assert workstyle["source"]["last_commit"] == manifest.last_commit

    office_hours = output_dir.joinpath("steps/office-hours.md").read_text(encoding="utf-8")
    assert "## Preamble" not in office_hours
    assert "## Voice" not in office_hours
    assert "skill-usage.jsonl" not in office_hours
    assert "spec-review.jsonl" not in office_hours
    assert "Do the work." in office_hours

    ceo_review_text = output_dir.joinpath("steps/ceo-review.md").read_text(encoding="utf-8")
    ceo_review = yaml.safe_load(ceo_review_text.split("---", 2)[1])
    assert ceo_review["after"] == ["office-hours"]
    assert "## Review Log" not in ceo_review_text

    browse = yaml.safe_load(
        output_dir.joinpath("steps/browse.md").read_text(encoding="utf-8").split("---", 2)[1]
    )
    assert browse["requires"] == ["browser"]
    assert "## SETUP" in output_dir.joinpath("steps/browse.md").read_text(encoding="utf-8")

    retro = output_dir.joinpath("steps/retro.md").read_text(encoding="utf-8")
    assert "skill-usage.jsonl" not in retro
    assert "eureka.jsonl" not in retro
    assert "Keep the actual retro." in retro

    direction = direction_output.read_text(encoding="utf-8")
    assert "You are GStack." in direction
    assert "Builders ship." in direction


def test_extract_openclaw_direction_drops_frontmatter(tmp_path: Path) -> None:
    soul = tmp_path / "SOUL.md"
    soul.write_text(
        """---
title: SOUL.md Template
---
# SOUL.md - Who You Are

Be helpful.
""",
        encoding="utf-8",
    )

    extracted = extract_openclaw_direction(soul)

    assert extracted == "# SOUL.md - Who You Are\n\nBe helpful.\n"


def test_convert_gstack_repo_rewrites_loopflow_references(tmp_path: Path) -> None:
    source_repo = tmp_path / "gstack"
    source_repo.mkdir()
    (source_repo / "SKILL.md").write_text(
        """---
name: gstack
description: Root skill.
---
## Voice

You are GStack.

# /gstack — Root

Root instructions.
""",
        encoding="utf-8",
    )

    (source_repo / "plan-eng-review").mkdir()
    (source_repo / "plan-eng-review" / "SKILL.md").write_text(
        """---
name: plan-eng-review
description: Review implementation plans after /office-hours.
---
## Voice

Planner voice.

# /plan-eng-review — Plan review

If needed, run /office-hours first, then continue with /plan-design-review.

Read the office-hours skill file from disk using the Read tool:
`~/.claude/skills/gstack/office-hours/SKILL.md`

Follow it inline, **skipping these sections** (already handled by the parent skill):
- Preamble (run first)
- Search Before Building
- Contributor Mode
- Telemetry (run last)

Then load:
- `~/.claude/skills/gstack/plan-design-review/SKILL.md`
""",
        encoding="utf-8",
    )

    _init_git_repo(source_repo)
    output_dir = tmp_path / "out"

    convert_gstack_repo(source_repo, output_dir)

    review = output_dir.joinpath("steps/eng-review.md").read_text(encoding="utf-8")
    frontmatter = yaml.safe_load(review.split("---", 2)[1])
    assert "gstack:office-hours" in review
    assert "gstack:design-review" in review
    assert ".lf/steps/gstack/office-hours.md" in review
    assert ".lf/steps/gstack/design-review.md" in review
    assert "Search Before Building" not in review
    assert "Contributor Mode" not in review
    assert "Telemetry (run last)" not in review
    assert frontmatter["description"] == "Review implementation plans after gstack:office-hours."


def _init_git_repo(repo: Path) -> None:
    subprocess.run(["git", "-C", str(repo), "init"], check=True, capture_output=True)
    subprocess.run(
        ["git", "-C", str(repo), "config", "user.name", "Loopflow Tests"],
        check=True,
        capture_output=True,
    )
    subprocess.run(
        ["git", "-C", str(repo), "config", "user.email", "tests@example.com"],
        check=True,
        capture_output=True,
    )
    subprocess.run(["git", "-C", str(repo), "add", "."], check=True, capture_output=True)
    subprocess.run(
        ["git", "-C", str(repo), "commit", "-m", "init"],
        check=True,
        capture_output=True,
    )
