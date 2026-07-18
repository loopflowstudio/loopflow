"""The published loopflow skill is valid, self-contained, and role-clear."""

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


def test_external_user_and_internal_worker_authority_are_distinct():
    skill = SKILL.read_text()
    agent_api = (ROOT / "docs/agent-api.md").read_text()
    readme = (ROOT / "README.md").read_text()

    for text in (skill, agent_api):
        assert "external harness" in text
        assert "Loopflow-launched" in text
        assert "User" in text
        assert "lf chat" in text

    install = "npx skills add loopflowstudio/loopflow --skill loopflow -g -y"
    assert install in agent_api
    assert install in readme
