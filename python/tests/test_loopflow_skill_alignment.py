"""The published loopflow skill is valid and portable."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SKILL = ROOT / "skills/loopflow/SKILL.md"

def test_skill_frontmatter_is_valid():
    text = SKILL.read_text()
    match = re.match(r"\A---\n(.*?)\n---\n", text, re.DOTALL)
    assert match, "SKILL.md must open with YAML frontmatter"
    frontmatter = match.group(1)
    assert re.search(r"^name: loopflow$", frontmatter, re.MULTILINE)
    description = re.search(r"^description: (.+)$", frontmatter, re.MULTILINE)
    assert description and len(description.group(1)) > 20


def test_skill_is_self_contained():
    """A published skill runs in repos that aren't this one: no repo-relative
    references outside the maintainer comment."""
    text = SKILL.read_text()
    body = re.sub(r"<!--.*?-->", "", text, flags=re.DOTALL)
    assert "rust/loopflow" not in body
    assert "scratch/website-docs-redo" not in body
