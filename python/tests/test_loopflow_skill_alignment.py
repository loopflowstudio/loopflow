"""The published loopflow skill is valid, self-contained, and aligned with LOOPFLOW.md."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SKILL = ROOT / "skills/loopflow/SKILL.md"
LOOPFLOW = ROOT / "rust/loopflow-definitions/src/builtins/LOOPFLOW.md"
DOCS = ROOT / "docs"


def _section(text: str, heading: str) -> str:
    pattern = rf"^## {re.escape(heading)}\n(.*?)(?=^## |\Z)"
    match = re.search(pattern, text, re.MULTILINE | re.DOTALL)
    assert match, f"missing section ## {heading}"
    return match.group(1).strip()


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


def test_how_to_work_matches_loopflow_doc_verbatim() -> None:
    assert _section(SKILL.read_text(), "How to work") == _section(
        LOOPFLOW.read_text(), "How to work"
    )


def test_routed_docs_topics_exist() -> None:
    skill = SKILL.read_text()
    topics = re.findall(r"`lf docs ([a-z0-9-]+)`", skill)
    assert topics, "SKILL.md should route to embedded `lf docs <topic>` pages"
    missing = [topic for topic in topics if not (DOCS / f"{topic}.md").is_file()]
    assert not missing, f"SKILL.md routes to missing docs pages: {missing}"


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


def test_launcher_surfaces_leave_wave_reads_to_help() -> None:
    for text in (SKILL.read_text(), LOOPFLOW.read_text()):
        assert "lf wave list --json" not in text
        assert "lf comment " not in text
        assert "lf task steer " not in text


def test_placeholders_never_sit_inside_double_quotes() -> None:
    """The person's words are data, never shell: a `"<…>"` placeholder teaches
    agents to paste text where the shell expands `$` and backticks."""
    for path in (SKILL, LOOPFLOW):
        found = re.findall(r'"<[^>"]+>"', path.read_text())
        assert not found, f"{path.name} double-quotes placeholders: {found}"
