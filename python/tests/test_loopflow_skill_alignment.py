"""The published loopflow skill is valid, self-contained, and role-clear."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SKILL = ROOT / "skills/loopflow/SKILL.md"
LOOPFLOW = ROOT / "rust/loopflow/src/builtins/LOOPFLOW.md"
WORK = ROOT / "rust/loopflow/src/builtins/ops/skill/work.md"


def test_skill_frontmatter_is_valid() -> None:
    text = SKILL.read_text()
    match = re.match(r"\A---\n(.*?)\n---\n", text, re.DOTALL)
    assert match, "SKILL.md must open with YAML frontmatter"
    frontmatter = match.group(1)
    assert re.search(r"^name: loopflow$", frontmatter, re.MULTILINE)
    description = re.search(r"^description: (.+)$", frontmatter, re.MULTILINE)
    assert description and len(description.group(1)) > 20


def test_skill_is_self_contained() -> None:
    """A published skill runs in repos that aren't this one: no repo-relative
    references outside the maintainer comment."""
    text = SKILL.read_text()
    body = re.sub(r"<!--.*?-->", "", text, flags=re.DOTALL)
    assert "rust/loopflow" not in body
    assert "scratch/website-docs-redo" not in body


def test_external_user_and_internal_worker_authority_are_distinct() -> None:
    skill = SKILL.read_text()
    agent_api = (ROOT / "docs/agent-api.md").read_text()
    readme = (ROOT / "README.md").read_text()

    assert "Never invent a Task identity or act on a Task nobody named" in skill
    assert "external harness" in agent_api
    assert "Loopflow-launched" in agent_api

    install = "npx skills add loopflowstudio/loopflow --skill loopflow -g"
    assert install in agent_api
    assert install in readme


def test_launcher_surfaces_load_the_shared_work_contract() -> None:
    for text in (LOOPFLOW.read_text(), (ROOT / "docs/agent-api.md").read_text()):
        assert "lf help skill work" in text

    for text in (SKILL.read_text(), LOOPFLOW.read_text()):
        assert "lf wave list --json" not in text

    work = WORK.read_text()
    assert "lf wave list --json" in work
    assert "lf wave status <wave> --json" in work
    assert "lf wave show --json" in work
