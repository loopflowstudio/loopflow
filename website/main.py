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
                Li(A("Features", href="/#features")),
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
                P("Loopflow, a software instrument."),
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
OWNERSHIP_CONTENT = _require_content_path("homepage.ownership")
LEVELS_CONTENT = _require_content_path("homepage.levels")
SHOWCASE_CONTENT = _require_content_path("homepage.showcase")
WHY_CONTENT = _require_content_path("homepage.why")
AUTONOMY_CONTENT = _require_content_path("homepage.autonomy")
INSTALL_CONTENT = _require_content_path("homepage.install")

for required_key in (
    "homepage.hero.tagline",
    "homepage.hero.subline",
    "homepage.hero.loopflow_download_url",
    "homepage.hero.example.steps",
    "homepage.ownership.items",
    "homepage.levels.items",
    "homepage.showcase.items",
    "homepage.showcase.pointer",
    "homepage.showcase.highlight",
    "homepage.why.claim",
    "homepage.why.paragraphs",
    "homepage.autonomy.levels",
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
        "Install Loopflow and take one piece of work from start to finish.",
        (
            DocPage("Overview", "index", "What Loopflow is, its pieces, and where to read next"),
            DocPage(
                "Get started",
                "getting-started",
                "Install it and take one piece of work from start to finish",
            ),
        ),
    ),
    DocArea(
        "Plan and conduct",
        "conduct",
        "Give Loopflow a goal to keep working on, then see where it stands and step in.",
        (
            DocPage("Waves", "waves", "Goals Loopflow keeps working on, and what they remember"),
            DocPage(
                "Conducting",
                "conducting",
                "See what got done, what needs you, and how to step in",
            ),
        ),
    ),
    DocArea(
        "Build and extend",
        "extend",
        "Change how Loopflow works: every step is a text file.",
        (
            DocPage("Authoring", "authoring", "Write your own steps, flows, and goals"),
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
        "Look up exact words, commands, settings, and fixes.",
        (
            DocPage("Glossary", "glossary", "Every term, in one plain sentence"),
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
> A software instrument that runs on your machine: durable Work, replaceable agents. lf is the CLI for daily work and the API agents call to run Skills, conduct Waves, deliver Tasks, and observe Home-local evidence.

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
# Markdown is what agents actually consume; HTML is the browser rendering.

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
    scope = f" from the {wave} wave" if wave else ""
    return P(
        f"Captured {captured_at}{scope} · Loopflow {app_version}",
        cls="loopflow-showcase-provenance",
    )


def _loopflow_diagram(example) -> FT:
    """The hero's example Flow: steps down a rail, one edge looping back, one exiting."""
    steps = example["steps"]
    rows = [(12 + 50 * index, step) for index, step in enumerate(steps)]
    last_y = rows[-1][0]
    loop_y = next(y for y, step in rows if step["name"] == example["loop_to"])

    def name_end(name: str) -> int:
        return 26 + round(len(name) * 7.9) + 12

    parts = [
        '<defs><marker id="loopflow-arrow" viewBox="0 0 10 10" refX="9" refY="5" '
        'markerWidth="7" markerHeight="7" orient="auto-start-reverse">'
        '<path d="M0 0L10 5L0 10z"/></marker></defs>'
    ]
    for y, step in rows:
        you = " you" if step["you"] else ""
        if y != last_y:
            parts.append(f'<line class="rail" x1="8" y1="{y + 8}" x2="8" y2="{y + 42}"/>')
        parts.append(
            f'<circle class="dot{you}" cx="8" cy="{y}" r="5.5"/>'
            f'<text class="name{you}" x="26" y="{y + 4}">{step["name"]}</text>'
            f'<text class="detail" x="26" y="{y + 21}">{step["detail"]}</text>'
        )
    parts.append(
        f'<path class="edge" d="M{name_end(steps[-1]["name"])} {last_y}H250V{loop_y}'
        f'H{name_end(example["loop_to"])}" marker-end="url(#loopflow-arrow)"/>'
        f'<text class="edge-label" transform="translate(266 {(last_y + loop_y) // 2}) rotate(90)" '
        f'text-anchor="middle">{example["loop_label"]}</text>'
        f'<path class="edge" d="M8 {last_y + 8}V{last_y + 50}" marker-end="url(#loopflow-arrow)"/>'
        f'<text class="edge-label" x="20" y="{last_y + 40}">{example["exit_label"]}</text>'
        f'<rect class="end" x="2.5" y="{last_y + 54.5}" width="11" height="11"/>'
        f'<text class="name" x="26" y="{last_y + 64}">{example["exit_to"]}</text>'
    )
    names = ", ".join(step["name"] for step in steps)
    label = (
        f"A loopflow: {names}. From {steps[-1]['name']}, one arrow returns to "
        f"{example['loop_to']} for {example['loop_label']} and one exits to {example['exit_to']}."
    )
    return NotStr(
        f'<svg class="home-loopflow" viewBox="0 0 280 {last_y + 78}" role="img" '
        f'aria-label="{label}">{"".join(parts)}</svg>'
    )


def _section_head(content, heading_id: str) -> FT:
    return Div(
        Div(
            P(content["label"], cls="home-label") if "label" in content else None,
            H2(content["heading"], id=heading_id),
        ),
        P(content["introduction"]),
        cls="home-section-head",
    )


def _part(part) -> FT:
    return Article(
        H3(part["title"]),
        P(part["description"], cls="feature-description"),
        A(part["link_label"], Span(" →", aria_hidden="true"), href=part["href"]),
        cls="home-part",
    )


def _levels_section() -> FT:
    return Section(
        Div(
            _section_head(LEVELS_CONTENT, "levels-heading"),
            Ol(
                *[
                    Li(
                        Div(P(item["label"], cls="home-label"), H3(item["title"])),
                        Div(
                            P(item["text"]),
                            Div(*[_part(part) for part in item["parts"]], cls="home-parts")
                            if "parts" in item
                            else None,
                            A(item["link_label"], Span(" →", aria_hidden="true"), href=item["href"])
                            if "href" in item
                            else None,
                        ),
                        cls="home-way",
                    )
                    for item in LEVELS_CONTENT["items"]
                ],
                cls="home-ways",
            ),
            cls="home-wrap",
        ),
        id="features",
        cls="home-levels-section",
        aria_labelledby="levels-heading",
    )


def _showcase_frames() -> list[dict]:
    """A frame renders only when its image exists; a missing capture never ships as a 404."""
    return [
        item
        for item in SHOWCASE_CONTENT["items"]
        if (STATIC_DIR / item["image"].removeprefix("/static/")).is_file()
    ]


def _showcase_section():
    frames = _showcase_frames()
    if not frames:
        return None
    pointer = SHOWCASE_CONTENT["pointer"]
    highlight = SHOWCASE_CONTENT["highlight"]
    position = (
        f"--pointer-left:{pointer['left']}%;--pointer-top:{pointer['top']}%;"
        f"--hit-left:{highlight['left']}%;--hit-top:{highlight['top']}%;"
        f"--hit-width:{highlight['width']}%;--hit-height:{highlight['height']}%"
    )
    provenance = []
    for frame in frames:
        sidecar = (STATIC_DIR / frame["image"].removeprefix("/static/")).with_suffix(".json")
        if (line := _provenance_line(sidecar)) is not None:
            provenance.append(line)
    return Section(
        Div(
            _section_head(SHOWCASE_CONTENT, "product-heading"),
            Div(
                *[
                    Img(
                        src=frame["image"],
                        alt=frame["image_alt"],
                        cls="home-shot is-current" if index == 0 else "home-shot",
                        data_frame=frame["id"],
                    )
                    for index, frame in enumerate(frames)
                ],
                Span(cls="home-stage-hit", aria_hidden="true"),
                Span(I(), cls="home-stage-pointer", aria_hidden="true"),
                cls="home-stage",
                style=position,
                data_stage="",
            ),
            Div(
                Div(
                    *[
                        Button(
                            Span(f"{index}", aria_hidden="true"),
                            f" {frame['label']}",
                            type="button",
                            data_frame=frame["id"],
                            data_caption=frame["caption"],
                            aria_pressed="true" if index == 1 else "false",
                        )
                        for index, frame in enumerate(frames, start=1)
                    ],
                    cls="home-stage-tabs",
                    role="group",
                    aria_label="Views",
                ),
                P(
                    frames[0]["caption"],
                    cls="home-stage-caption",
                    aria_live="polite",
                    data_caption="",
                ),
                cls="home-stage-bar",
            ),
            Div(
                P(SHOWCASE_CONTENT["note"]),
                *provenance,
                P(
                    "Full size: ",
                    *[A(frame["label"], href=frame["image"]) for frame in frames],
                    cls="home-stage-links",
                ),
                cls="home-stage-note",
            ),
            Div(
                *[P(B(item["title"]), f" {item['text']}") for item in SHOWCASE_CONTENT["notes"]],
                cls="home-stage-points",
            ),
            cls="home-wrap",
        ),
        id="product",
        cls="home-showcase",
        aria_labelledby="product-heading",
    )


def _autonomy_section() -> FT:
    link = AUTONOMY_CONTENT["link"]
    return Section(
        Div(
            Div(
                H2(AUTONOMY_CONTENT["heading"], id="autonomy-heading"),
                P(AUTONOMY_CONTENT["introduction"]),
                cls="home-section-head",
            ),
            Div(
                *[
                    Div(
                        Div(Span(level["label"], cls="home-label"), H3(level["title"])),
                        Ol(
                            *[
                                Li(step["name"], cls="you" if step["you"] else None)
                                for step in level["steps"]
                            ],
                            cls="home-staff",
                        ),
                        P(level["description"]),
                        cls="home-level",
                    )
                    for level in AUTONOMY_CONTENT["levels"]
                ],
                cls="home-levels",
            ),
            P(
                Span(f"● {AUTONOMY_CONTENT['legend_you']}"),
                Span(f"○ {AUTONOMY_CONTENT['legend_agent']}"),
                A(link["label"], Span(" →", aria_hidden="true"), href=link["href"]),
                cls="home-legend",
            ),
            cls="home-wrap",
        ),
        cls="home-autonomy",
        aria_labelledby="autonomy-heading",
    )


def build_homepage():
    loopflow_download_url = HERO_CONTENT["loopflow_download_url"]
    example = HERO_CONTENT["example"]
    install_display = INSTALL_CONTENT["command_display"].strip()
    install_copy = INSTALL_CONTENT["command_copy"]

    return (
        Title("Loopflow, a software instrument"),
        SkipLink(),
        Navbar(),
        Main(
            # Hero — headline and one sentence, beside an example loopflow
            Section(
                Div(
                    Div(
                        H1(HERO_CONTENT["tagline"]),
                        P(HERO_CONTENT["subline"], cls="tagline hero-subline"),
                        Div(
                            A(
                                "Download for Mac",
                                href=loopflow_download_url,
                                cls="btn btn-primary",
                            ),
                            A("Read the docs", href="/docs", cls="btn btn-secondary"),
                            cls="hero-actions",
                        ),
                    ),
                    Aside(
                        P(example["label"], cls="home-label"),
                        _loopflow_diagram(example),
                        P(example["caption"]),
                        P(example["file"], cls="home-file"),
                        cls="home-example",
                    ),
                    cls="home-wrap home-hero-grid",
                ),
                cls="hero home-hero",
            ),
            Section(
                Div(
                    *[
                        Div(H2(item["title"]), P(item["text"]))
                        for item in OWNERSHIP_CONTENT["items"]
                    ],
                    cls="home-wrap home-owned-grid",
                ),
                cls="home-owned",
                aria_label="Ownership",
            ),
            _levels_section(),
            _showcase_section(),
            Section(
                Div(
                    P(WHY_CONTENT["label"], cls="home-label"),
                    Div(
                        H2(WHY_CONTENT["claim"]),
                        *[
                            P(B(WHY_CONTENT["lead"]), f" {paragraph}")
                            if index == 0
                            else P(paragraph)
                            for index, paragraph in enumerate(WHY_CONTENT["paragraphs"])
                        ],
                    ),
                    cls="home-wrap home-why-grid",
                ),
                id="why",
                cls="home-why",
            ),
            _autonomy_section(),
            # Install — the app first; the command line tool rides along
            Section(
                Div(
                    Div(
                        P(INSTALL_CONTENT["label"], cls="home-label"),
                        H2(INSTALL_CONTENT["heading"], cls="quick-install-heading"),
                        Div(
                            A(
                                "Download for Mac",
                                href=loopflow_download_url,
                                cls="btn btn-primary",
                            ),
                            A("Read the docs", href="/docs", cls="btn btn-secondary"),
                            cls="hero-actions",
                        ),
                        P(INSTALL_CONTENT["note"], cls="install-note"),
                        P(INSTALL_CONTENT["requirement"], cls="install-note"),
                    ),
                    Div(
                        P(INSTALL_CONTENT["cli_label"], cls="install-note"),
                        Div(
                            Pre(Code(install_display), cls="install-code", tabindex="0"),
                            CopyButton(install_copy),
                            cls="install-code-wrapper",
                        ),
                    ),
                    cls="home-wrap home-install-grid",
                ),
                cls="quick-install home-install",
            ),
            id="main-content",
            cls="home",
        ),
        SiteFooter(),
        Script(src=f"/static/home.js?v={STYLE_VERSION}", defer=True),
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
                    P("Runs on your computer. Nothing to register.", cls="tagline"),
                    Div(
                        Div(
                            H2("Mac app"),
                            P(
                                "Start here. The app shows your goals, your tasks, and what needs you, "
                                "and it includes the command line tool.",
                                cls="install-desc",
                            ),
                            A(
                                "Download for Mac",
                                href=HERO_CONTENT["loopflow_download_url"],
                                cls="btn btn-primary",
                            ),
                            cls="mac-app-option",
                        ),
                        H2("Command line"),
                        P(
                            "The same tool without the app, for macOS or Linux. "
                            "Everything the app does, it does by running these commands.",
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
                        Div(
                            P("Then, inside a project:", cls="next-skill-label"),
                            Pre(
                                Code("cd your-project\nlf init"),
                                cls="install-code next-skills",
                                tabindex="0",
                            ),
                            cls="next-skills-wrapper",
                        ),
                        P(
                            "Before you start: an AI coding tool (Claude Code or Codex), signed in, "
                            "and a project that uses Git.",
                            cls="system-req",
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
