import difflib
import json
import os
import re
from datetime import datetime, timezone
from pathlib import Path

import yaml
from fasthtml.common import *
from starlette.responses import PlainTextResponse, RedirectResponse
from starlette.routing import Route

from internal_pages import colors_page, design_page, fonts_page

BASE_URL = "https://loopflow.studio"
REPO_URL = "https://github.com/loopflowstudio/loopflow"

app, rt = fast_app(
    htmlkw={"lang": "en"},
    hdrs=(
        Meta(name="viewport", content="width=device-width, initial-scale=1"),
        Link(rel="icon", href="/static/logo.svg", type="image/svg+xml"),
        NotStr(
            '<meta name="viewport" content="width=device-width, initial-scale=1.0, viewport-fit=cover">'
        ),
        Link(
            rel="stylesheet",
            href="https://fonts.googleapis.com/css2?family=Instrument+Serif:ital@0;1&family=Inter:wght@400;500;600&family=JetBrains+Mono:wght@400;500&display=swap",
        ),
        Link(rel="stylesheet", href="/static/style.css"),
    ),
)


# Components


def SkipLink():
    return A("Skip to main content", href="#main-content", cls="skip-link")


def Navbar():
    return Nav(
        Div(
            Div(
                A(
                    Img(src="/static/logo.svg", alt="Loopflow"),
                    href="/",
                    cls="nav-logo",
                ),
                A("Loopflow", href="/", cls="nav-title"),
                cls="nav-brand-group",
            ),
            Ul(
                Li(A("Docs", href="/docs")),
                Li(
                    A(
                        "GitHub",
                        href="https://github.com/loopflowstudio/loopflow",
                        target="_blank",
                        rel="noopener noreferrer",
                        cls="external-link",
                        **{"aria-label": "GitHub (opens in new tab)"},
                    )
                ),
                Li(A("Install", href="/download", cls="btn btn-primary")),
                cls="nav-links",
            ),
            cls="container",
        ),
        **{"aria-label": "Main navigation"},
    )


def SiteFooter():
    return Footer(
        Div(
            Div(
                P("Loopflow — Living software. Conducted by you."),
                P(
                    "Built by ",
                    A(
                        "Loopflow Studio",
                        href="https://github.com/loopflowstudio",
                        target="_blank",
                        rel="noopener noreferrer",
                        cls="external-link",
                        **{"aria-label": "Loopflow Studio (opens in new tab)"},
                    ),
                    cls="built-by",
                ),
                cls="footer-text",
            ),
            Div(
                A("Docs", href="/docs"),
                A("GitHub", href="https://github.com/loopflowstudio/loopflow"),
                cls="footer-links",
            ),
            cls="container",
        ),
    )


def CopyButton(text: str):
    return Button(
        NotStr(
            '<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M15.75 17.25v3.375c0 .621-.504 1.125-1.125 1.125h-9.75a1.125 1.125 0 0 1-1.125-1.125V7.875c0-.621.504-1.125 1.125-1.125H6.75a9.06 9.06 0 0 1 1.5.124m7.5 10.376h3.375c.621 0 1.125-.504 1.125-1.125V11.25c0-4.46-3.243-8.161-7.5-8.876a9.06 9.06 0 0 0-1.5-.124H9.375c-.621 0-1.125.504-1.125 1.125v3.5m7.5 10.375H9.375a1.125 1.125 0 0 1-1.125-1.125v-9.25m12 6.625v-1.875a3.375 3.375 0 0 0-3.375-3.375h-1.5a1.125 1.125 0 0 1-1.125-1.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H9.75" /></svg>'
        ),
        cls="copy-btn",
        onclick=f"navigator.clipboard.writeText('{text}')",
        **{"aria-label": "Copy to clipboard"},
    )


def CodeBlock(filename: str, content: str):
    return Div(
        Div(filename, cls="code-block-header"),
        Pre(Code(content), tabindex="0"),
        cls="code-block",
    )


def CapabilityItem(title: str, description: str):
    return Div(H3(title), P(render_inline(description)), cls="capability-item")


# Load content from YAML (edit content.yaml, not this file)
CONTENT_FILE = Path(__file__).parent / "content.yaml"
_content = yaml.safe_load(CONTENT_FILE.read_text())


def _require_content_path(path: str) -> object:
    current = _content
    for segment in path.split("."):
        if not isinstance(current, dict) or segment not in current:
            raise RuntimeError(f"content.yaml missing required key: {path}")
        current = current[segment]
    return current


# Homepage
HERO_CONTENT = _require_content_path("homepage.hero")
SHOWCASE_CONTENT = _require_content_path("homepage.showcase")
PILLARS_CONTENT = _require_content_path("homepage.pillars")
BUILDING_BLOCKS_CONTENT = _require_content_path("homepage.building_blocks")
INSTALL_CONTENT = _require_content_path("homepage.install")

for required_key in (
    "homepage.hero.tagline",
    "homepage.hero.subline",
    "homepage.hero.loopflow_download_url",
    "homepage.showcase.heading",
    "homepage.showcase.items",
    "homepage.pillars.items",
    "homepage.building_blocks.items",
    "homepage.install.command_display",
    "homepage.install.command_copy",
):
    _require_content_path(required_key)

# Markdown rendering

DOCS_DIR = Path(__file__).parent / "docs"
CANONICAL_DOCS_DIR = Path(__file__).parent.parent / "docs"
STATIC_DIR = Path(__file__).parent / "static"


def slugify(text: str) -> str:
    """Convert heading text to URL-friendly slug: 'Quick Reference' -> 'quick-reference'"""
    slug = text.lower()
    slug = re.sub(r"[^a-z0-9\s-]", "", slug)
    slug = re.sub(r"\s+", "-", slug)
    slug = re.sub(r"-+", "-", slug)
    return slug.strip("-")


DOCS_NAV = [
    ("Home", "index"),
    ("Get Started", "getting-started"),
    ("Waves", "waves"),
    ("Authoring", "authoring"),
    ("The Agent API", "agent-api"),
    ("Conducting", "conducting"),
    ("Architecture", "architecture"),
    # Reference
    ("lf", "lf"),
    ("Config", "config"),
    ("Troubleshooting", "troubleshooting"),
]


DOC_DESCRIPTIONS = {
    "index": "What loopflow is, the model, and where to read next",
    "getting-started": "Install, first commands, building features, going remote",
    "waves": "The planning model, goals, memory, KRs, Linear, crons",
    "authoring": "Writing skills, flows, directions, and goals",
    "agent-api": "How agents launch, steer, and prove control of other agents",
    "conducting": "Monitoring and steering many agents; the Mac podium",
    "architecture": "No server: the store, the journal, Homes, lf ssh, lfd",
    "lf": "Every command, PR/planning/release operations, the builtin catalog",
    "config": "Config files, context assembly, models, accounts and profiles",
    "troubleshooting": "Exact failure → cause → fix",
}


def generate_llms_txt() -> str:
    """llms.txt per llmstxt.org: H1, blockquote summary, context, H2 link sections."""
    doc_links = "\n".join(
        f"- [{title}]({BASE_URL}/docs/{slug}.md): {DOC_DESCRIPTIONS.get(slug, title)}"
        for title, slug in DOCS_NAV
    )
    return f"""# Loopflow
> Persistent agents, no server. Waves hold a goal, remember what they learn, and stay steerable — and lf is the command humans type and the API agents call to launch, steer, and observe other agents.

Loopflow creates and runs Waves: each coordinates Linear-backed Projects and
Tasks, keeps one steerable conversation beside the live work map, and folds
what it learns into memory. State lives in a local SQLite store and
append-only journals; shared truth lives in Linear and GitHub; remote
machines are reached over SSH. Install: `curl -fsSL
https://loopflow.studio/install.sh | sh && lf init`. Every docs page below is
raw markdown at its `.md` URL (or request the canonical URL with `Accept:
text/markdown`); the complete corpus is at {BASE_URL}/llms-full.txt.

## Docs

{doc_links}

## Optional

- [GitHub](https://github.com/loopflowstudio/loopflow): source, releases, install.sh
- [Release notes](https://github.com/loopflowstudio/loopflow/blob/main/RELEASE_NOTES.md): the full chronology
"""


def generate_llms_full_txt() -> str:
    """The whole docs corpus in one markdown file, in nav order."""
    sections = []
    for title, slug in DOCS_NAV:
        body = load_doc(slug)
        if not body:
            continue
        sections.append(f"<!-- {BASE_URL}/docs/{slug} -->\n\n{body.strip()}")
    header = (
        "# Loopflow — complete documentation\n\n"
        f"> Concatenation of every page under {BASE_URL}/docs, in reading order. "
        f"Curated index: {BASE_URL}/llms.txt\n"
    )
    return header + "\n\n---\n\n".join(sections) + "\n"


def generate_sitemap_xml() -> str:
    pages = ["", "/download", "/docs"] + [
        f"/docs/{slug}" for _, slug in DOCS_NAV if slug != "index"
    ]
    entries = []
    for page in pages:
        lastmod = ""
        slug = page.removeprefix("/docs/") if page.startswith("/docs/") else None
        path = doc_path(slug or "index") if (slug or page == "/docs") else None
        if path:
            date = datetime.fromtimestamp(path.stat().st_mtime, tz=timezone.utc).date()
            lastmod = f"<lastmod>{date.isoformat()}</lastmod>"
        entries.append(f"<url><loc>{BASE_URL}{page}</loc>{lastmod}</url>")
    body = "\n".join(entries)
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
        f"{body}\n</urlset>\n"
    )


# (Generated at startup below, after the doc-loading helpers are defined.)


def render_markdown(content: str) -> list:
    # Remove YAML frontmatter
    content = re.sub(r"^---\n.*?\n---\n", "", content, flags=re.DOTALL)

    elements = []
    lines = content.split("\n")
    i = 0

    while i < len(lines):
        line = lines[i]

        # Code blocks
        if line.startswith("```"):
            code_lines = []
            i += 1
            while i < len(lines) and not lines[i].startswith("```"):
                code_lines.append(lines[i])
                i += 1
            elements.append(Pre(Code("\n".join(code_lines)), tabindex="0"))
            i += 1
            continue

        # Headers
        if line.startswith("# "):
            elements.append(H1(line[2:]))
            i += 1
            continue
        if line.startswith("## "):
            heading_text = line[3:]
            anchor_id = slugify(heading_text)
            elements.append(
                H2(
                    heading_text,
                    A(
                        "#",
                        href=f"#{anchor_id}",
                        cls="anchor-link",
                        **{"aria-label": "Link to section"},
                    ),
                    id=anchor_id,
                )
            )
            i += 1
            continue
        if line.startswith("### "):
            heading_text = line[4:]
            anchor_id = slugify(heading_text)
            elements.append(
                H3(
                    heading_text,
                    A(
                        "#",
                        href=f"#{anchor_id}",
                        cls="anchor-link",
                        **{"aria-label": "Link to section"},
                    ),
                    id=anchor_id,
                )
            )
            i += 1
            continue

        # Horizontal rule
        if line.strip() == "---":
            elements.append(Hr())
            i += 1
            continue

        # Tables
        if "|" in line and i + 1 < len(lines) and "---" in lines[i + 1]:
            headers = [h.strip() for h in line.split("|") if h.strip()]
            i += 2  # Skip header and separator
            rows = []
            while i < len(lines) and "|" in lines[i]:
                cells = [c.strip() for c in lines[i].split("|") if c.strip()]
                rows.append(cells)
                i += 1
            elements.append(
                Table(
                    Thead(Tr(*[Th(h) for h in headers])),
                    Tbody(*[Tr(*[Td(render_inline(c)) for c in row]) for row in rows]),
                )
            )
            continue

        # Unordered lists
        if line.startswith("- "):
            items = []
            while i < len(lines) and lines[i].startswith("- "):
                items.append(Li(render_inline(lines[i][2:])))
                i += 1
            elements.append(Ul(*items))
            continue

        # Ordered lists
        if re.match(r"^\d+\. ", line):
            items = []
            while i < len(lines) and re.match(r"^\d+\. ", lines[i]):
                items.append(Li(render_inline(re.sub(r"^\d+\. ", "", lines[i]))))
                i += 1
            elements.append(Ol(*items))
            continue

        # Blockquotes
        if line.startswith("> "):
            quote_lines = []
            while i < len(lines) and lines[i].startswith("> "):
                quote_lines.append(lines[i][2:])
                i += 1
            elements.append(Blockquote(P(" ".join(quote_lines))))
            continue

        # Paragraphs
        if line.strip():
            # Always consume the current line: a prose line may contain "|"
            # (it wasn't a table — that case is handled above) and must not
            # stall the loop.
            para_lines = [line]
            i += 1
            while (
                i < len(lines)
                and lines[i].strip()
                and not lines[i].startswith("#")
                and not lines[i].startswith("```")
                and not lines[i].startswith("- ")
                and not lines[i].startswith("> ")
                and "|" not in lines[i]
            ):
                para_lines.append(lines[i])
                i += 1
            elements.append(P(render_inline(" ".join(para_lines))))
            continue

        i += 1

    return elements


def render_inline(text: str) -> NotStr:
    # Handle images - convert relative paths to /static/
    def fix_image(m):
        alt, src = m.group(1), m.group(2)
        if not src.startswith(("http", "/")):
            src = "/static/" + src
        return f'<img src="{src}" alt="{alt}">'

    text = re.sub(r"!\[([^\]]*)\]\(([^)]+)\)", fix_image, text)

    # Handle links - convert relative .md links to absolute /docs/ paths
    def fix_link(m):
        label, href = m.group(1), m.group(2)
        if href.endswith(".md") and not href.startswith(("http", "/")):
            href = "/docs/" + href
        elif ".md#" in href and not href.startswith(("http", "/")):
            href = "/docs/" + href.replace(".md#", "#")
        return f'<a href="{href}">{label}</a>'

    text = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", fix_link, text)
    # Handle inline code
    text = re.sub(r"`([^`]+)`", r"<code>\1</code>", text)
    # Handle bold
    text = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", text)
    # Handle italic
    text = re.sub(r"\*([^*]+)\*", r"<em>\1</em>", text)
    return NotStr(text)


def DocsNav(current: str = "index"):
    return Nav(
        P("Documentation", cls="docs-nav-heading"),
        Ul(
            *[
                Li(
                    A(
                        title,
                        href=f"/docs/{slug}" if slug != "index" else "/docs",
                        cls="active" if slug == current else None,
                        **({"aria-current": "page"} if slug == current else {}),
                    )
                )
                for title, slug in DOCS_NAV
            ]
        ),
        cls="docs-nav",
        **{"aria-label": "Documentation"},
    )


def doc_path(slug: str) -> Path | None:
    for docs_dir in (DOCS_DIR, CANONICAL_DOCS_DIR):
        path = docs_dir / f"{slug}.md"
        if path.exists():
            return path
    return None


def load_doc(slug: str) -> str:
    path = doc_path(slug)
    return path.read_text() if path else ""


# Agent-facing markdown delivery: every docs page is retrievable as raw
# markdown — /docs/<slug>.md, or Accept: text/markdown on the canonical URL.
# Markdown is what agents actually consume; HTML is the human rendering.

MARKDOWN_MEDIA_TYPE = "text/markdown; charset=utf-8"


def _doc_title(slug: str) -> str:
    return next((t for t, s in DOCS_NAV if s == slug), slug.title())


def markdown_doc_response(slug: str) -> PlainTextResponse | None:
    path = doc_path(slug)
    if not path:
        return None
    updated = datetime.fromtimestamp(path.stat().st_mtime, tz=timezone.utc)
    frontmatter = (
        "---\n"
        f"title: {_doc_title(slug)}\n"
        f"canonical_url: {BASE_URL}/docs/{slug}\n"
        f"last_updated: {updated.date().isoformat()}\n"
        "---\n\n"
    )
    return PlainTextResponse(
        frontmatter + path.read_text(),
        media_type=MARKDOWN_MEDIA_TYPE,
        headers={"Vary": "Accept"},
    )


def markdown_not_found(slug: str) -> PlainTextResponse:
    """Markdown 404 with nearest-match suggestions — agents recover; HTML error shells dead-end them."""
    slugs = [s for _, s in DOCS_NAV]
    close = difflib.get_close_matches(slug, slugs, n=3, cutoff=0.4) or slugs
    suggestions = "\n".join(f"- {BASE_URL}/docs/{s}.md" for s in close)
    body = (
        f"# Not found: /docs/{slug}\n\n"
        f"Closest pages:\n\n{suggestions}\n\n"
        f"Full index: {BASE_URL}/llms.txt\n"
    )
    return PlainTextResponse(
        body,
        status_code=404,
        media_type=MARKDOWN_MEDIA_TYPE,
        headers={"Vary": "Accept"},
    )


def wants_markdown(request) -> bool:
    # Browsers never ask for text/markdown; any client that does gets it.
    return "text/markdown" in request.headers.get("accept", "")


# Generated at startup for caching
LLMS_TXT_CONTENT = generate_llms_txt()
LLMS_FULL_TXT_CONTENT = generate_llms_full_txt()
SITEMAP_XML_CONTENT = generate_sitemap_xml()


# Pages


def _capture_figure(item):
    """Render only a complete image + provenance + live-status triple."""
    image = item["image"]
    image_path = STATIC_DIR / image.removeprefix("/static/")
    sidecar_path = image_path.with_suffix(".json")
    status_snapshot = image.removesuffix(".png") + ".status.json"
    if not all(
        path.is_file()
        for path in (image_path, sidecar_path, image_path.with_suffix(".status.json"))
    ):
        return None
    try:
        provenance = json.loads(sidecar_path.read_text())
        captured_at = provenance["captured_at"][:10]
        wave = provenance["wave"]
        app_version = provenance["app_version"]
        app_commit = provenance["app_commit"]
    except (KeyError, TypeError, json.JSONDecodeError):
        return None
    capture_label = f"Captured {captured_at} from the {wave} wave"
    build_label = f"Loopflow {app_version} @ {app_commit[:7]}"
    return Figure(
        Img(
            src=image,
            alt=item["image_alt"],
            cls="loopflow-showcase-img",
        ),
        Figcaption(
            P(item["caption"]),
            P(
                A(
                    capture_label,
                    href=status_snapshot,
                    **{"aria-label": f"{capture_label}; inspect the live status snapshot"},
                ),
                " · ",
                A(
                    build_label,
                    href=f"{REPO_URL}/commit/{app_commit}",
                    **{"aria-label": f"{build_label}; inspect the source commit"},
                ),
                cls="loopflow-showcase-provenance",
            ),
            cls="loopflow-showcase-caption",
        ),
        cls="loopflow-showcase-figure",
    )


def _screenshot_section():
    """A missing or unproven capture never renders."""
    figures = [figure for item in SHOWCASE_CONTENT["items"] if (figure := _capture_figure(item))]
    if not figures:
        return None
    return Section(
        Div(
            H2(SHOWCASE_CONTENT["heading"]),
            Div(*figures, cls="loopflow-showcase-grid"),
            cls="container",
        ),
        cls="loopflow-showcase-section",
    )


def build_homepage():
    loopflow_download_url = HERO_CONTENT["loopflow_download_url"]
    install_display = INSTALL_CONTENT["command_display"].strip()
    install_copy = INSTALL_CONTENT["command_copy"]
    pillar_items = PILLARS_CONTENT["items"]
    building_blocks = BUILDING_BLOCKS_CONTENT["items"]

    return (
        Title("Loopflow — Living software, conducted by you"),
        SkipLink(),
        Navbar(),
        Main(
            # Hero — logo, headline, tagline, CTAs
            Section(
                Div(
                    Img(src="/static/logo.svg", alt="Loopflow", cls="hero-logo-large"),
                    H1("Loopflow"),
                    P(HERO_CONTENT["tagline"], cls="tagline"),
                    P(HERO_CONTENT["subline"], cls="hero-subline"),
                    Div(
                        A("Download for Mac", href=loopflow_download_url, cls="btn btn-primary"),
                        A("Read the docs", href="/docs", cls="btn btn-secondary"),
                        cls="btn-group hero-actions",
                    ),
                    cls="container hero-centered",
                ),
                cls="hero",
            ),
            # Demo — the Context Lab capture, when present
            _screenshot_section(),
            # Pillars
            Section(
                Div(
                    H2(PILLARS_CONTENT["heading"]),
                    Div(
                        *[
                            CapabilityItem(item["title"], item["description"])
                            for item in pillar_items
                        ],
                        cls="capabilities-grid",
                    ),
                ),
                cls="capabilities-section",
            ),
            # Building blocks — wave → project → task → skill
            Section(
                Div(
                    H2(BUILDING_BLOCKS_CONTENT["heading"]),
                    Div(
                        *[
                            Div(
                                P(item["label"], cls="building-block-label"),
                                CodeBlock(item["filename"], item["content"].strip()),
                                cls="building-block-item",
                            )
                            for item in building_blocks
                        ],
                        cls="building-blocks-grid",
                    ),
                ),
                cls="building-blocks-section",
            ),
            # Bottom CTA — the app first; the CLI rides along
            Section(
                Div(
                    H2(INSTALL_CONTENT["heading"], cls="quick-install-heading"),
                    Div(
                        A("Download for Mac", href=loopflow_download_url, cls="btn btn-primary"),
                        A("Read the docs", href="/docs", cls="btn btn-secondary"),
                        cls="hero-actions",
                    ),
                    P(INSTALL_CONTENT["note"], cls="install-note"),
                    P(INSTALL_CONTENT["cli_label"], cls="install-note"),
                    Div(
                        Pre(Code(install_display), cls="install-code", tabindex="0"),
                        CopyButton(install_copy),
                        cls="install-code-wrapper",
                    ),
                    cls="container",
                ),
                cls="quick-install",
            ),
            id="main-content",
        ),
        SiteFooter(),
    )


@rt("/")
def get():
    return build_homepage()


@rt("/install.sh")
def get():
    return RedirectResponse(
        "https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh",
        status_code=302,
    )


@rt("/cli")
def get():
    # Redirect /cli to docs
    return RedirectResponse("/docs", status_code=302)


@rt("/products")
def get():
    # Redirect /products to home
    return RedirectResponse("/", status_code=302)


@rt("/loopflow")
def get():
    # The old product page; the app now lives on /download
    return RedirectResponse("/download", status_code=302)


@rt("/maestro")
def get():
    # Redirect /maestro to download
    return RedirectResponse("/download", status_code=302)


@rt("/team")
def get():
    return RedirectResponse("/", status_code=302)


@rt("/story")
def get():
    return RedirectResponse("/", status_code=302)


@rt("/agents")
def get():
    # Redirect /agents to docs
    return RedirectResponse("/docs", status_code=302)


def _docs_page(slug: str, title: str):
    content = load_doc(slug)
    return (
        Title(title),
        SkipLink(),
        Navbar(),
        Main(
            Section(
                Div(
                    DocsNav(slug),
                    Div(
                        P(
                            A(
                                "View as Markdown",
                                href=f"/docs/{slug}.md",
                                cls="md-link",
                                title="This page as raw markdown, for agents and copying",
                            ),
                            cls="docs-md-link",
                        ),
                        *render_markdown(content),
                        cls="docs-content",
                    ),
                    cls="docs-layout",
                ),
                cls="docs-hero",
            ),
            id="main-content",
        ),
        SiteFooter(),
    )


@rt("/docs")
def get(request):
    if wants_markdown(request):
        return markdown_doc_response("index")
    return (*_docs_page("index", "Loopflow Documentation"), HttpHeader("Vary", "Accept"))


@rt("/docs/{slug:path}")
def get(request, slug: str):
    # Raw markdown: /docs/<slug>.md, or Accept: text/markdown on the canonical URL
    if slug.endswith(".md"):
        slug = slug[:-3]
        return markdown_doc_response(slug) or markdown_not_found(slug)
    if wants_markdown(request):
        return markdown_doc_response(slug) or markdown_not_found(slug)
    if not doc_path(slug):
        return RedirectResponse("/docs", status_code=302)
    title = _doc_title(slug)
    return (
        *_docs_page(slug, f"{title} — Loopflow Documentation"),
        HttpHeader("Vary", "Accept"),
    )


@rt("/download")
def get():
    return (
        Title("Install Loopflow"),
        SkipLink(),
        Navbar(),
        Main(
            Section(
                Div(
                    Img(src="/static/logo.svg", alt="Loopflow", cls="hero-logo"),
                    H1("Install"),
                    P("One binary. No server. Nothing to register.", cls="tagline"),
                    Div(
                        H2("CLI"),
                        P(
                            "The command humans type and the API agents call. Best for: daily work, waves, and everything headless.",
                            cls="install-desc",
                        ),
                        Div(
                            Pre(
                                Code("curl -fsSL https://loopflow.studio/install.sh | sh"),
                                cls="install-code",
                                tabindex="0",
                            ),
                            CopyButton("curl -fsSL https://loopflow.studio/install.sh | sh"),
                            cls="install-code-wrapper",
                        ),
                        P("macOS or Linux · Claude Code, Codex, or OpenCode", cls="system-req"),
                        Div(
                            P("Then:", cls="next-skill-label"),
                            Pre(
                                Code("cd your-project\nlf init\nlf debug -c"),
                                cls="install-code next-skills",
                                tabindex="0",
                            ),
                            cls="next-skills-wrapper",
                        ),
                        Div(
                            H2("Mac app"),
                            P(
                                "The podium: wave chat, the machine-wide roadmap, and every task's worktree — a pure client over the same local state.",
                                cls="install-desc",
                            ),
                            A(
                                "Download for Mac",
                                href=HERO_CONTENT["loopflow_download_url"],
                                cls="btn btn-secondary",
                            ),
                            cls="mac-app-option",
                        ),
                        cls="install-option",
                        style="max-width: 420px; margin: 0 auto;",
                    ),
                    cls="container",
                ),
                cls="hero hero-centered download-hero",
            ),
            id="main-content",
        ),
        SiteFooter(),
    )


@rt("/fonts")
def get():
    return fonts_page()


@rt("/colors")
def get():
    return colors_page()


@rt("/design")
def get():
    return design_page()


def _llms_txt_handler(request):
    return PlainTextResponse(LLMS_TXT_CONTENT, media_type="text/plain")


def _llms_full_txt_handler(request):
    return PlainTextResponse(LLMS_FULL_TXT_CONTENT, media_type="text/plain")


def _sitemap_handler(request):
    return PlainTextResponse(SITEMAP_XML_CONTENT, media_type="application/xml")


# Insert machine-readable routes at the beginning to avoid the static handler
app.routes.insert(0, Route("/llms.txt", _llms_txt_handler, methods=["GET"]))
app.routes.insert(0, Route("/llms-full.txt", _llms_full_txt_handler, methods=["GET"]))
app.routes.insert(0, Route("/sitemap.xml", _sitemap_handler, methods=["GET"]))


@rt("/favicon.ico")
async def favicon():
    """Serve logo.svg as favicon for browsers that request .ico."""
    return FileResponse("static/logo.svg", media_type="image/svg+xml")


@rt("/static/{fname:path}")
async def static(fname: str):
    return FileResponse(f"static/{fname}")


if __name__ == "__main__":
    serve(host="0.0.0.0", port=int(os.environ.get("PORT", 5001)))
