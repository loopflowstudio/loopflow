from __future__ import annotations

import subprocess
from pathlib import Path

import yaml

from loopflow.workstyle.convert import convert_gstack_repo, extract_openclaw_direction


def test_convert_gstack_repo_writes_steps_manifest_and_direction(tmp_path: Path) -> None:
    source_repo = tmp_path / "gstack"
    source_repo.mkdir()

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

Do the work.

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
    assert manifest.steps == ["browse", "office-hours", "ceo-review"]

    workstyle = yaml.safe_load(output_dir.joinpath("workstyle.yaml").read_text(encoding="utf-8"))
    assert workstyle["prefix"] == "gstack"
    assert workstyle["source"]["repo"] == "garrytan/gstack"
    assert workstyle["source"]["last_commit"] == manifest.last_commit

    office_hours = output_dir.joinpath("steps/office-hours.md").read_text(encoding="utf-8")
    assert "## Preamble" not in office_hours
    assert "## Voice" not in office_hours
    assert "Do the work." in office_hours

    ceo_review = yaml.safe_load(
        output_dir.joinpath("steps/ceo-review.md").read_text(encoding="utf-8").split("---", 2)[1]
    )
    assert ceo_review["after"] == ["office-hours"]

    browse = yaml.safe_load(
        output_dir.joinpath("steps/browse.md").read_text(encoding="utf-8").split("---", 2)[1]
    )
    assert browse["requires"] == ["browser"]
    assert "## SETUP" in output_dir.joinpath("steps/browse.md").read_text(encoding="utf-8")

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
