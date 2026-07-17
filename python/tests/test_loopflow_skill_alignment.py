"""The published loopflow skill tracks the injected operating contract.

skills/loopflow/SKILL.md is the LOOPFLOW.md contract reframed for agents not
launched by lf. The two are worded differently on purpose, but the doctrine
anchors — headings, load-bearing sentences, and command spellings — must
appear in both. When this fails, the builtin changed: re-align the skill.
"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BUILTIN = ROOT / "rust/loopflow/src/engine/builtins/LOOPFLOW.md"
SKILL = ROOT / "skills/loopflow/SKILL.md"

# Exact strings that carry the shared doctrine. Substring match in both files.
DOCTRINE_ANCHORS = [
    "Execute Here First",
    "Checkpoint And Proceed",
    "Delegation must make the problem smaller",
    "only when the active skill or the human explicitly asks for orchestration",
    "sibling naming convention (`<repo>.<name>`)",
    "`lf chat` is the User surface",
    "lf commit -m",
    "lf pr publish",
    "lf pr submit",
    "lf pr land",
    "lf pr land -c",
    "lf rebase --plan",
    "--stack-on",
    "lf radio pub",
    "lf memory add",
    "lf top",
]


def test_doctrine_anchors_appear_in_both():
    # Whitespace-normalized so hard-wrapped lines still match.
    builtin = " ".join(BUILTIN.read_text().split())
    skill = " ".join(SKILL.read_text().split())
    missing = [
        (anchor, name)
        for anchor in DOCTRINE_ANCHORS
        for name, text in (("builtin", builtin), ("skill", skill))
        if anchor not in text
    ]
    assert not missing, (
        "Doctrine anchors missing — the builtin contract and the published "
        f"skill have drifted apart: {missing}. "
        "Re-align skills/loopflow/SKILL.md with the builtin (or update the "
        "anchor list if the doctrine itself changed in both)."
    )


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
