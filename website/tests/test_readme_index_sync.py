"""The README and docs/index.md open with the same words, on purpose.

README.md is the GitHub front door; docs/index.md is the site front door.
Their opening (everything between the H1 and the first section heading) is
one text maintained in both places — this test is the drift alarm. Edit one,
copy to the other.
"""

import re
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent.parent


def _opening(path: Path) -> str:
    text = path.read_text()
    match = re.search(r"^# Loopflow\n(.*?)^## ", text, re.DOTALL | re.MULTILINE)
    assert match, f"{path.name} lacks an H1-to-first-section opening"
    return " ".join(match.group(1).split())


def test_readme_and_index_share_their_opening():
    readme = _opening(REPO_ROOT / "README.md")
    index = _opening(REPO_ROOT / "docs" / "index.md")
    assert readme == index, (
        "README.md and docs/index.md openings have drifted apart. "
        "They are maintained as one text — copy the intended version to both."
    )
