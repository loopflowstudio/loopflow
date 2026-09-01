import difflib
import hashlib
import json
import os
import posixpath
import re
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urlsplit, urlunsplit

import yaml
from fasthtml.common import *
from markdown import markdown as markdown_to_html
from starlette.responses import JSONResponse, PlainTextResponse, RedirectResponse
from starlette.routing import Route

from internal_pages import colors_page, design_page, fonts_page

BASE_URL = "https://loopflow.studio"
RELEASE_TAG = os.environ.get("LOOPFLOW_RELEASE_TAG", "development")
STATIC_DIR = Path(__file__).parent / "static"
STYLE_VERSION = hashlib.sha256((STATIC_DIR / "style.css").read_bytes()).hexdigest()[:12]

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
        Link(rel="stylesheet", href=f"/static/style.css?v={STYLE_VERSION}"),
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


def slugify(text: str) -> str:
    """Convert heading text to URL-friendly slug: 'Quick Reference' -> 'quick-reference'"""
    slug = text.lower()
    slug = re.sub(r"[^a-z0-9\s-]", "", slug)
    slug = re.sub(r"\s+", "-", slug)
    slug = re.sub(r"-+", "-", slug)
    return slug.strip("-")


@dataclass(frozen=True)
class DocPage:
    title: str
    slug: str
    description: str
    parent: str | None = None


@dataclass(frozen=True)
class DocArea:
    title: str
    slug: str
    description: str
    pages: tuple[DocPage, ...]


DOCS_AREAS = (
    DocArea(
        "Start",
        "start",
        "Install Loopflow and follow one useful command all the way through.",
        (
            DocPage("Overview", "index", "The model and where to read next"),
            DocPage(
                "Get started",
                "getting-started",
                "Install, run a Skill, build a feature, and go remote",
            ),
        ),
    ),
    DocArea(
        "Plan and conduct",
        "conduct",
        "Give long-running work a purpose, then observe and redirect it.",
        (
            DocPage("Waves", "waves", "Goals, memory, Projects, Tasks, KRs, and cadence"),
            DocPage(
                "Conducting",
                "conducting",
                "Monitor and steer work from the CLI, tmux, or Mac app",
            ),
        ),
    ),
    DocArea(
        "Build and extend",
        "extend",
        "Turn your own operating knowledge into reusable Skills and agent workflows.",
        (
            DocPage("Authoring", "authoring", "Write Skills, Flows, directions, and goals"),
            DocPage(
                "The Agent API",
                "agent-api",
                "Launch, steer, observe, and ship work from another agent",
            ),
        ),
    ),
    DocArea(
        "Reference",
        "reference",
        "Look up exact commands, settings, identities, boundaries, and repairs.",
        (
            DocPage("lf command reference", "lf", "Commands, flags, and builtins"),
            DocPage("Configuration", "config", "Context, models, profiles, and launch behavior"),
            DocPage(
                "Subscriptions",
                "subscriptions",
                "Provider identities, routes, health, and remote selection",
            ),
            DocPage(
                "Security",
                "security",
                "Execution, credential, storage, and network trust boundaries",
            ),
            DocPage("Troubleshooting", "troubleshooting", "Exact failure, cause, and fix"),
        ),
    ),
)

ARCHITECTURE_AREA = DocArea(
    "Developer architecture",
    "architecture",
    "Follow one Skill run through the implementation, then open the subsystem you need.",
    (
        DocPage("Architecture", "architecture", "A developer's path through the whole system"),
        DocPage(
            "Execution",
            "architecture/execution",
            "Skill discovery, prompts, providers, harnesses, and Run evidence",
            parent="architecture",
        ),
        DocPage(
            "Planning",
            "architecture/planning",
            "Flows, Work, Steer, Ask, and resident loops",
            parent="architecture",
        ),
        DocPage(
            "Delivery",
            "architecture/delivery",
            "Task worktrees, commits, serial PRs, checks, and merge",
            parent="architecture",
        ),
        DocPage(
            "Homes and processes",
            "architecture/homes",
            "Placement, services, SSH, process authority, and promotion",
            parent="architecture",
        ),
        DocPage(
            "Data and persistence",
            "architecture/data",
            "Truth owners, stores, append-only evidence, and projections",
            parent="architecture",
        ),
        DocPage(
            "Codebase map",
            "architecture/codebase",
            "Source territories, public surfaces, binaries, and extension points",
            parent="architecture",
        ),
        DocPage(
            "Checked reference",
            "architecture-reference",
            "The exhaustive, machine-checked inventory",
            parent="architecture",
        ),
    ),
)

DOC_PAGES = tuple(page for area in DOCS_AREAS for page in area.pages)
PUBLIC_DOC_SLUGS = {page.slug for page in DOC_PAGES}
ARCHITECTURE_PAGES = ARCHITECTURE_AREA.pages
ARCHITECTURE_SLUGS = {page.slug for page in ARCHITECTURE_PAGES}
ALL_DOC_PAGES = DOC_PAGES + ARCHITECTURE_PAGES
DOCS_NAV = [(page.title, page.slug) for page in DOC_PAGES]
DOC_DESCRIPTIONS = {page.slug: page.description for page in DOC_PAGES}
DOC_PAGE_BY_SLUG = {page.slug: page for page in ALL_DOC_PAGES}
DOC_AREA_BY_PAGE = {
    page.slug: area for area in DOCS_AREAS for page in area.pages
}
DOC_AREA_BY_PAGE.update(
    {page.slug: ARCHITECTURE_AREA for page in ARCHITECTURE_PAGES}
)


def generate_llms_txt() -> str:
    """llms.txt per llmstxt.org: H1, blockquote summary, context, H2 link sections."""
    doc_links = "\n".join(
        f"- [{title}]({BASE_URL}/docs/{slug}.md): {DOC_DESCRIPTIONS.get(slug, title)}"
        for title, slug in DOCS_NAV
        if doc_path(slug)
    )
    return f"""# Loopflow
> Durable Work, replaceable agents, no central server. lf is the command humans type and the API agents call to run Skills, conduct Waves, deliver Tasks, and observe Home-local evidence.

Loopflow runs one Skill through a provider and records that launch in an
immutable Home-local Run record. Stable Wave, Project, and Task Work preserves
purpose across provider processes. Authored behavior lives in the repository;
bounded planning state lives on its Home; shared planning and delivery truth
lives in Linear and GitHub. Reach another Home explicitly with `lf ssh`.
Install: `curl -fsSL
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
        f"/docs/{slug}"
        for _, slug in DOCS_NAV
        if slug != "index" and doc_path(slug)
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


def _strip_frontmatter(content: str) -> str:
    return re.sub(r"^---\r?\n.*?\r?\n---\r?\n", "", content, count=1, flags=re.DOTALL)


def _split_doc_source(content: str) -> tuple[str, str]:
    source = _strip_frontmatter(content).lstrip()
    match = re.match(r"# ([^\n]+)\n?", source)
    if not match:
        return "Documentation", source
    return match.group(1).strip(), source[match.end() :].lstrip()


def _resolve_doc_target(
    target: str, current_slug: str, architecture: bool = False
) -> str:
    if target.startswith(("#", "/", "http://", "https://", "mailto:")):
        return target

    parsed = urlsplit(target)
    source_dir = posixpath.dirname(current_slug)
    resolved = posixpath.normpath(posixpath.join(source_dir, parsed.path))

    if parsed.path.endswith(".md"):
        if resolved.startswith("../"):
            repo_path = resolved.removeprefix("../")
            path = f"https://github.com/loopflowstudio/loopflow/blob/main/{repo_path}"
        else:
            slug = resolved.removesuffix(".md")
            path = (
                _architecture_href(slug)
                if architecture and slug in ARCHITECTURE_SLUGS
                else ("/docs" if slug == "index" else f"/docs/{slug}")
            )
        return urlunsplit(("", "", path, parsed.query, parsed.fragment))

    if resolved.startswith("../"):
        repo_path = resolved.removeprefix("../")
        path = f"https://github.com/loopflowstudio/loopflow/blob/main/{repo_path}"
        return urlunsplit(("", "", path, parsed.query, parsed.fragment))

    if Path(parsed.path).suffix.lower() in {".gif", ".jpeg", ".jpg", ".png", ".svg", ".webp"}:
        path = f"/static/{resolved}"
        return urlunsplit(("", "", path, parsed.query, parsed.fragment))

    return target


def _resolve_markdown_targets(
    content: str, current_slug: str, architecture: bool = False
) -> str:
    pattern = re.compile(r"(!?\[[^\]]*\]\()([^)\s]+)([^)]*\))")

    def replace(match: re.Match[str]) -> str:
        href = _resolve_doc_target(match.group(2), current_slug, architecture)
        return f"{match.group(1)}{href}{match.group(3)}"

    return pattern.sub(replace, content)


def render_markdown(
    content: str, current_slug: str = "index", architecture: bool = False
) -> list:
    source = _resolve_markdown_targets(
        _strip_frontmatter(content), current_slug, architecture
    )
    html = markdown_to_html(
        source,
        extensions=("fenced_code", "sane_lists", "tables", "toc"),
        extension_configs={
            "toc": {
                "permalink": "#",
                "permalink_class": "anchor-link",
                "permalink_title": "Link to this section",
            }
        },
        output_format="html5",
    )
    html = html.replace("<pre>", '<pre tabindex="0">')
    html = html.replace("<table>", '<table tabindex="0">')
    return [NotStr(html)]


def _doc_outline(content: str) -> list[tuple[str, str]]:
    source = _strip_frontmatter(content)
    headings = []
    in_fence = False
    for line in source.splitlines():
        if line.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence or not line.startswith("## "):
            continue
        title = re.sub(r"[`*_]", "", line[3:]).strip()
        headings.append((title, slugify(title)))
    return headings


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
    groups = []
    for area in DOCS_AREAS:
        pages = [page for page in area.pages if doc_path(page.slug)]
        if not pages:
            continue
        groups.append(
            Div(
                P(area.title, cls="docs-nav-area-title"),
                Ul(
                    *[
                        Li(
                            A(
                                page.title,
                                href=(
                                    f"/docs/{page.slug}"
                                    if page.slug != "index"
                                    else "/docs"
                                ),
                                cls="active" if page.slug == current else None,
                                **(
                                    {"aria-current": "page"}
                                    if page.slug == current
                                    else {}
                                ),
                            ),
                            cls="docs-nav-child" if page.parent else None,
                        )
                        for page in pages
                    ]
                ),
                cls="docs-nav-area",
                **{"data-area": area.slug},
            )
        )
    current_page = DOC_PAGE_BY_SLUG.get(current)
    return Nav(
        Details(
            Summary(
                Span("Browse docs"),
                Span(current_page.title if current_page else "Documentation"),
            ),
            Div(
                A(
                    Span("Loopflow", cls="docs-nav-wordmark"),
                    Span("Documentation", cls="docs-nav-heading"),
                    href="/docs",
                    cls="docs-nav-home",
                ),
                *groups,
                cls="docs-nav-inner",
            ),
            cls="docs-nav-disclosure",
            open=True,
        ),
        cls="docs-nav",
        **{"aria-label": "Documentation"},
    )


def _architecture_href(slug: str, markdown: bool = False) -> str:
    if slug == "architecture":
        suffix = ""
    elif slug == "architecture-reference":
        suffix = "/reference"
    else:
        suffix = f"/{slug.removeprefix('architecture/')}"
    return f"/architecture{suffix}{'.md' if markdown else ''}"


def ArchitectureNav(current: str):
    pages = [page for page in ARCHITECTURE_PAGES if doc_path(page.slug)]
    current_page = DOC_PAGE_BY_SLUG[current]
    return Nav(
        Details(
            Summary(Span("Browse architecture"), Span(current_page.title)),
            Div(
                A(
                    Span("Loopflow source", cls="docs-nav-wordmark"),
                    Span("Developer architecture", cls="docs-nav-heading"),
                    href="/architecture",
                    cls="docs-nav-home",
                ),
                Div(
                    P("Follow the system", cls="docs-nav-area-title"),
                    Ul(
                        *[
                            Li(
                                A(
                                    page.title,
                                    href=_architecture_href(page.slug),
                                    cls="active" if page.slug == current else None,
                                    **(
                                        {"aria-current": "page"}
                                        if page.slug == current
                                        else {}
                                    ),
                                ),
                                cls="docs-nav-child" if page.parent else None,
                            )
                            for page in pages
                        ]
                    ),
                    cls="docs-nav-area",
                ),
                A("Public user docs ↗", href="/docs", cls="docs-nav-public-link"),
                cls="docs-nav-inner",
            ),
            cls="docs-nav-disclosure",
            open=True,
        ),
        cls="docs-nav docs-nav-architecture",
        **{"aria-label": "Developer architecture"},
    )


def DocsBreadcrumb(slug: str, title: str):
    page = DOC_PAGE_BY_SLUG.get(slug)
    area = DOC_AREA_BY_PAGE.get(slug)
    crumbs = [Li(A("Docs", href="/docs"))]
    if area and slug != "index":
        crumbs.append(Li(Span(area.title)))
    if page and page.parent:
        parent = DOC_PAGE_BY_SLUG[page.parent]
        crumbs.append(Li(A(parent.title, href=f"/docs/{parent.slug}")))
    if slug != "index":
        crumbs.append(Li(Span(title, **{"aria-current": "page"})))
    return Nav(
        Ol(*crumbs),
        cls="docs-breadcrumb",
        **{"aria-label": "Breadcrumb"},
    )


def ArchitectureBreadcrumb(slug: str, title: str):
    if slug == "architecture":
        return None
    crumbs = [Li(A("Architecture", href="/architecture"))]
    crumbs.append(Li(Span(title, **{"aria-current": "page"})))
    return Nav(
        Ol(*crumbs),
        cls="docs-breadcrumb",
        **{"aria-label": "Breadcrumb"},
    )


def DocsOutline(content: str):
    headings = _doc_outline(content)
    if len(headings) < 2:
        return None
    return Nav(
        P("On this page", cls="docs-outline-heading"),
        Ol(*[Li(A(title, href=f"#{anchor}")) for title, anchor in headings]),
        cls="docs-outline",
        **{"aria-label": "On this page"},
    )


def DocsDirectory():
    return Section(
        H2("Browse by area", id="browse-by-area"),
        Ol(
            *[
                Li(
                    Span(f"{index:02}", cls="docs-directory-number"),
                    Div(
                        H3(area.title),
                        P(area.description),
                        Ul(
                            *[
                                Li(
                                    A(
                                        page.title,
                                        href=(
                                            f"/docs/{page.slug}"
                                            if page.slug != "index"
                                            else "/docs"
                                        ),
                                    )
                                )
                                for page in area.pages
                                if doc_path(page.slug) and page.slug != "index"
                            ]
                        ),
                    ),
                )
                for index, area in enumerate(DOCS_AREAS, start=1)
            ],
            cls="docs-directory-list",
        ),
        cls="docs-directory",
        **{"aria-labelledby": "browse-by-area"},
    )


def DocsPager(
    current: str,
    pages: tuple[DocPage, ...] = DOC_PAGES,
    architecture: bool = False,
):
    pages = [page for page in pages if doc_path(page.slug)]
    index = next((i for i, page in enumerate(pages) if page.slug == current), None)
    if index is None:
        return None
    previous = pages[index - 1] if index > 0 else None
    following = pages[index + 1] if index + 1 < len(pages) else None

    def pager_link(page: DocPage, direction: str):
        return A(
            Span(direction, cls="docs-pager-direction"),
            Span(page.title, cls="docs-pager-title"),
            href=(
                _architecture_href(page.slug)
                if architecture
                else (f"/docs/{page.slug}" if page.slug != "index" else "/docs")
            ),
            cls=f"docs-pager-link docs-pager-{direction.lower()}",
        )

    return Nav(
        pager_link(previous, "Previous") if previous else None,
        pager_link(following, "Next") if following else None,
        cls="docs-pager",
        **{"aria-label": "Documentation pages"},
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
    page = DOC_PAGE_BY_SLUG.get(slug)
    return page.title if page else slug.title()


def markdown_doc_response(
    slug: str, canonical_path: str | None = None
) -> PlainTextResponse | None:
    path = doc_path(slug)
    if not path:
        return None
    updated = datetime.fromtimestamp(path.stat().st_mtime, tz=timezone.utc)
    frontmatter = (
        "---\n"
        f"title: {_doc_title(slug)}\n"
        f"canonical_url: {BASE_URL}{canonical_path or f'/docs/{slug}'}\n"
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


def _provenance_line(sidecar_path: Path):
    """The caption's provenance line, only when the sidecar exists and parses."""
    if not sidecar_path.is_file():
        return None
    try:
        provenance = json.loads(sidecar_path.read_text())
        captured_at = provenance["captured_at"][:10]
        wave = provenance["wave"]
        app_version = provenance["app_version"]
    except (KeyError, TypeError, json.JSONDecodeError):
        return None
    return P(
        f"Captured {captured_at} from the {wave} wave · Loopflow {app_version}",
        cls="loopflow-showcase-provenance",
    )


def _capture_figure(item):
    """A figure renders whenever its image exists; provenance rides along when proven."""
    image = item["image"]
    image_path = STATIC_DIR / image.removeprefix("/static/")
    if not image_path.is_file():
        return None
    caption_parts = [P(item["caption"])]
    provenance = _provenance_line(image_path.with_suffix(".json"))
    if provenance is not None:
        caption_parts.append(provenance)
    return Figure(
        Img(
            src=image,
            alt=item["image_alt"],
            cls="loopflow-showcase-img",
        ),
        Figcaption(*caption_parts, cls="loopflow-showcase-caption"),
        cls="loopflow-showcase-figure",
    )


def _screenshot_section():
    """One product shot under the hero; a missing capture never renders."""
    for item in SHOWCASE_CONTENT["items"]:
        figure = _capture_figure(item)
        if figure is not None:
            return Section(Div(figure, cls="container"), cls="loopflow-showcase-section")
    return None


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
            # Demo — one product capture, when present
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


def _docs_page(slug: str, title: str, architecture: bool = False):
    content = load_doc(slug)
    heading, body = _split_doc_source(content)
    page = DOC_PAGE_BY_SLUG.get(slug)
    return (
        Title(title),
        *(
            (Meta(name="robots", content="noindex,nofollow"),)
            if architecture
            else ()
        ),
        SkipLink(),
        Script(src="/static/docs.js", defer=True),
        Navbar(),
        Main(
            Section(
                Div(
                    ArchitectureNav(slug) if architecture else DocsNav(slug),
                    Article(
                        (
                            ArchitectureBreadcrumb(slug, heading)
                            if architecture
                            else DocsBreadcrumb(slug, heading)
                        ),
                        Header(
                            Div(
                                P(
                                    A(
                                        "Markdown source",
                                        href=(
                                            _architecture_href(slug, markdown=True)
                                            if architecture
                                            else f"/docs/{slug}.md"
                                        ),
                                        cls="md-link",
                                        title="This page as raw markdown, for agents and copying",
                                    ),
                                    cls="docs-md-link",
                                ),
                                cls="docs-page-utility",
                            ),
                            H1(heading),
                            P(page.description, cls="docs-deck") if page else None,
                            cls="docs-page-header",
                        ),
                        DocsDirectory() if slug == "index" and not architecture else None,
                        *render_markdown(body, slug, architecture=architecture),
                        DocsPager(
                            slug,
                            pages=ARCHITECTURE_PAGES if architecture else DOC_PAGES,
                            architecture=architecture,
                        ),
                        cls="docs-content",
                    ),
                    DocsOutline(body),
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
        if slug not in PUBLIC_DOC_SLUGS:
            return markdown_not_found(slug)
        return markdown_doc_response(slug) or markdown_not_found(slug)
    if wants_markdown(request):
        if slug not in PUBLIC_DOC_SLUGS:
            return markdown_not_found(slug)
        return markdown_doc_response(slug) or markdown_not_found(slug)
    if slug not in PUBLIC_DOC_SLUGS or not doc_path(slug):
        return RedirectResponse("/docs", status_code=302)
    title = _doc_title(slug)
    return (
        *_docs_page(slug, f"{title} — Loopflow Documentation"),
        HttpHeader("Vary", "Accept"),
    )


def _architecture_source_slug(route_slug: str) -> str | None:
    route_slug = route_slug.removesuffix(".md").strip("/")
    if not route_slug:
        return "architecture"
    if route_slug == "reference":
        return "architecture-reference"
    candidate = f"architecture/{route_slug}"
    return candidate if candidate in ARCHITECTURE_SLUGS else None


def _architecture_page(request, route_slug: str):
    markdown = route_slug.endswith(".md") or wants_markdown(request)
    source_slug = _architecture_source_slug(route_slug)
    if source_slug is None or not doc_path(source_slug):
        if markdown:
            return PlainTextResponse(
                "# Architecture page not found\n",
                status_code=404,
                media_type=MARKDOWN_MEDIA_TYPE,
            )
        return RedirectResponse("/architecture", status_code=302)
    canonical = _architecture_href(source_slug)
    if markdown:
        return markdown_doc_response(source_slug, canonical_path=canonical)
    return (
        *_docs_page(
            source_slug,
            f"{_doc_title(source_slug)} — Loopflow Developer Architecture",
            architecture=True,
        ),
        HttpHeader("Vary", "Accept"),
        HttpHeader("X-Robots-Tag", "noindex, nofollow"),
    )


@rt("/architecture.md")
def get(request):
    return _architecture_page(request, ".md")


@rt("/architecture")
def get(request):
    return _architecture_page(request, "")


@rt("/architecture/{slug:path}")
def get(request, slug: str):
    return _architecture_page(request, slug)


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
                    P("Local-first. No central server. Nothing to register.", cls="tagline"),
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


def _healthz_handler(request):
    return JSONResponse({"status": "ok", "release": RELEASE_TAG})


# Insert machine-readable routes at the beginning to avoid the static handler
app.routes.insert(0, Route("/llms.txt", _llms_txt_handler, methods=["GET"]))
app.routes.insert(0, Route("/llms-full.txt", _llms_full_txt_handler, methods=["GET"]))
app.routes.insert(0, Route("/sitemap.xml", _sitemap_handler, methods=["GET"]))
app.routes.insert(0, Route("/healthz", _healthz_handler, methods=["GET"]))


@rt("/favicon.ico")
async def favicon():
    """Serve logo.svg as favicon for browsers that request .ico."""
    return FileResponse("static/logo.svg", media_type="image/svg+xml")


@rt("/static/{fname:path}")
async def static(fname: str):
    return FileResponse(f"static/{fname}")


if __name__ == "__main__":
    serve(host="0.0.0.0", port=int(os.environ.get("PORT", 5001)))
