import subprocess
import sys
from pathlib import Path


def test_portable_architecture_matches_the_internal_surface():
    root = Path(__file__).parents[2]
    subprocess.run(
        [sys.executable, str(root / "scripts" / "render_architecture_html.py"), "--check"],
        cwd=root,
        check=True,
    )
    html = (root / "docs" / "architecture.html").read_text()
    assert "Loopflow Developer Architecture" in html
    assert "Home-local Run record" in html
    assert 'href="https://loopflow.studio/architecture/execution"' in html
    assert 'href="/static/style.css"' not in html
