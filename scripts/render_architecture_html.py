#!/usr/bin/env python3
"""Render the portable developer-architecture entry from the website surface."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WEBSITE = ROOT / "website"
OUTPUT = ROOT / "docs" / "architecture.html"

sys.path.insert(0, str(WEBSITE))

import main as site  # noqa: E402
from fasthtml.common import to_xml  # noqa: E402


def render_html() -> str:
    fragments = site._docs_page(
        "architecture",
        "Architecture — Loopflow Developer Architecture",
        architecture=True,
    )
    body = "\n".join(to_xml(fragment) for fragment in fragments[2:])
    body = re.sub(
        r'(?P<attr>href|src)="/(?!/)',
        rf'\g<attr>="{site.BASE_URL}/',
        body,
    )
    css = "\n".join(
        (
            (WEBSITE / "static" / "style.css").read_text(),
            (ROOT / "docs" / "architecture.css").read_text(),
        )
    )
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="generator" content="scripts/render_architecture_html.py">
  <meta name="robots" content="noindex,nofollow">
  <title>Architecture — Loopflow Developer Architecture</title>
  <link rel="icon" href="{site.BASE_URL}/static/logo.svg" type="image/svg+xml">
  <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Instrument+Serif:ital@0;1&family=Inter:wght@400;500;600&family=JetBrains+Mono:wght@400;500&display=swap">
  <style>
{css}
  </style>
</head>
<body class="portable-docs">
{body}
</body>
</html>
"""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when docs/architecture.html is not current",
    )
    args = parser.parse_args()
    rendered = render_html()
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text() != rendered:
            raise SystemExit(
                "docs/architecture.html is stale; run scripts/render_architecture_html.py"
            )
        return
    OUTPUT.write_text(rendered)
    print(f"wrote {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
