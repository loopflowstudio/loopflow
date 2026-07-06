import os
import re
from pathlib import Path

import yaml
from fasthtml.common import *
from starlette.responses import PlainTextResponse, RedirectResponse
from starlette.routing import Route

from db import add_to_waitlist, init_db
from internal_pages import colors_page, design_page, fonts_page

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
        Script(src="https://unpkg.com/htmx.org@1.9.10"),
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
                Li(A("Team", href="/team")),
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


def TeamCard(name: str, title: str, bio: str, image_src: str | None = None):
    photo = Img(src=image_src, alt=name, cls="team-photo") if image_src else None
    return Div(
        photo,
        Div(
            H3(name),
            P(title, cls="team-title"),
            P(bio, cls="team-bio"),
            cls="team-card-body",
        ),
        cls="team-card",
    )


def TerminalBlock(lines: list[tuple[str, str]]):
    """lines is a list of (type, content) tuples where type is 'command', 'output', or 'comment'"""
    return Div(
        Div(
            Span(cls="terminal-dot red", **{"aria-hidden": "true"}),
            Span(cls="terminal-dot yellow", **{"aria-hidden": "true"}),
            Span(cls="terminal-dot green", **{"aria-hidden": "true"}),
            cls="terminal-header",
        ),
        Div(
            *[Div(line[1], cls=f"terminal-line {line[0]}") for line in lines],
            cls="terminal-body",
            tabindex="0",
            role="group",
            **{"aria-label": "Terminal output"},
        ),
        cls="terminal",
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


CHEVRON_SVG = '<svg class="term-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M19 9l-7 7-7-7" /></svg>'
VOCAB_ACCORDION_SCRIPT = """
    document.querySelectorAll('.vocab-grid').forEach(grid => {
        grid.addEventListener('toggle', event => {
            if (!event.target.open) return;
            grid.querySelectorAll('details').forEach(card => {
                if (card !== event.target) card.open = false;
            });
        }, true);
    });
"""


def TermCard(number: str, name: str, icon_svg: str, essence: str, details: list[tuple[str, str]]):
    """Vocabulary card with expandable details."""
    summary_content = [
        Span(number, cls="term-number"),
        NotStr(f'<div class="term-icon" aria-hidden="true">{icon_svg}</div>'),
        H3(name),
        P(essence, cls="term-essence"),
        NotStr(CHEVRON_SVG),
    ]
    detail_items = [
        Div(
            Div(label, cls="term-detail-label"),
            Div(content, cls="term-detail-text"),
            cls="term-detail-item",
        )
        for label, content in details
    ]

    return Details(
        Summary(*summary_content),
        Div(*detail_items, cls="term-details-content"),
        cls="term-card",
    )


def DemoVideo(poster: str, src: str | None = None, cls: str = "demo-video"):
    attrs = {
        "poster": poster,
        "autoplay": True,
        "muted": True,
        "loop": True,
        "playsinline": True,
        "preload": "none",
        "cls": cls,
        "aria-hidden": "true",
    }
    if src:
        attrs["data-src"] = src
    return Video(**attrs)


def CapabilityItem(title: str, description: str):
    return Div(H3(title), P(description), cls="capability-item")


def _detail_li(text: str):
    if " — " in text:
        bold, rest = text.split(" — ", 1)
        return Li(Strong(bold), f" — {rest}")
    return Li(text)


def ProductCard(
    name: str, description: str, details: list[str], image_content, note: str | None = None
):
    header = [H3(name)]
    if note:
        header.append(Span(note, cls="product-note"))
    return Div(
        image_content,
        Div(*header, cls="product-card-header"),
        P(description),
        Ul(*[_detail_li(detail) for detail in details]),
        cls="product-card",
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
VIDEO_CONTENT = _require_content_path("homepage.video")
INSTALL_CONTENT = _require_content_path("homepage.install")
CAPABILITIES_CONTENT = _require_content_path("homepage.capabilities")
BUILDING_BLOCKS_CONTENT = _require_content_path("homepage.building_blocks")
PRODUCTS_CONTENT = _require_content_path("homepage.products")

for required_key in (
    "homepage.hero.tagline",
    "homepage.hero.concerto_download_url",
    "homepage.video.poster",
    "homepage.install.command_display",
    "homepage.install.command_copy",
    "homepage.capabilities.items",
    "homepage.building_blocks.items",
    "homepage.products.concerto",
    "homepage.products.server",
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


DOCS_NAV = [
    ("Home", "index"),
    ("Waves", "waves"),
    ("Wave Authoring", "wave-authoring"),
    ("Get Started", "getting-started"),
    # Reference
    ("lf", "lf"),
    ("lf op", "lfop"),
    ("lfd", "lfd"),
    ("Config", "config"),
    ("Troubleshooting", "troubleshooting"),
]


def generate_llms_txt() -> str:
    """Generate machine-readable doc summary for /llms.txt"""
    return """# Loopflow
> Living software, conducted by you. Waves of autonomous work that remember, react, and adapt.

## What is Loopflow?
Loopflow builds living codebases. Waves — persistent units of autonomous work — loop through your code,
remember what they learn (via MEMORY.md), react to file changes and other waves, and adapt their prompts
to the surface (terminal, desktop, mobile). You control context and quality through composable building blocks.

## Install
curl -fsSL https://loopflow.studio/install.sh | sh && lf init

## Commands
lf <step>           Run a step (design, implement, review, etc.)
lf debug -c         Fix error from clipboard
lf op pr            Create PR from current branch
lf op wt create X   Create worktree for feature X
lf wave X           Start wave X's server (its resident mind)
lf q worker run X --flow build --task "..."   Dispatch a PR-producing worker
lf chat -w X "..."  Post into wave X's thread

## Core Concepts
Wave: a named agent with a goal. Authored as wave/<name>/GOAL.md (intent + loop
  prompt) and wave/<name>/MEMORY.md (durable memory the agent writes).
Wave agent: coordinates — reads roadmap and memory, decides the next move,
  dispatches workers, folds results back into memory. Rarely writes code itself.
Worker: a scoped agent a wave dispatches to run a flow and open a PR; inherits
  the wave's GOAL.md and MEMORY.md.
Step: a prompt that runs a coding agent. Flow: a sequence of steps.
Direction: composable quality definitions that shape agent judgment.
Roadmap: the wave's work queue, provider-backed (e.g. Linear).

## Docs
/docs              Overview and quick start
/docs/waves        Waves (goal agents, memory, workers)
/docs/lf           lf command reference
/docs/lfop         lf op command reference
/docs/lfd          lfd (daemon) reference
/docs/config       Configuration options

## Configuration
Config file: .lf/config.yaml
Steps: .lf/steps/*.md or .claude/commands/*.md
Directions: .lf/directions/*.md
Memory: wave/*/MEMORY.md (per-wave, auto-managed)

## Links
GitHub: https://github.com/loopflowstudio/loopflow
Docs: https://loopflow.studio/docs
"""


# Generate llms.txt content at startup for caching
LLMS_TXT_CONTENT = generate_llms_txt()


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
            para_lines = []
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


def load_doc(slug: str) -> str:
    for docs_dir in (DOCS_DIR, CANONICAL_DOCS_DIR):
        path = docs_dir / f"{slug}.md"
        if path.exists():
            return path.read_text()
    return ""


# Pages

# SVG icons (shared across sections)
_ICON_BOLT = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M3.75 13.5l10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75z" /></svg>'
_ICON_SWAP = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M7.5 21 3 16.5m0 0L7.5 12M3 16.5h13.5m0-13.5L21 7.5m0 0L16.5 12M21 7.5H7.5" /></svg>'
_ICON_LOOP = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.992 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182m0-4.991v4.99" /></svg>'

HOMEPAGE_VIDEO_BEHAVIOR_SCRIPT = """
    document.addEventListener('DOMContentLoaded', () => {
        const videos = document.querySelectorAll('video[data-src]');
        const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

        if (prefersReducedMotion) {
            document.querySelectorAll('video[autoplay]').forEach((video) => {
                video.removeAttribute('autoplay');
                video.pause();
            });
        }

        if (!videos.length) return;

        if (!('IntersectionObserver' in window)) {
            videos.forEach((video) => {
                if (!video.getAttribute('src')) {
                    video.src = video.dataset.src;
                }
            });
            return;
        }

        const observer = new IntersectionObserver((entries) => {
            entries.forEach((entry) => {
                if (!entry.isIntersecting) return;
                const video = entry.target;
                if (!video.getAttribute('src')) {
                    video.src = video.dataset.src;
                }
                observer.unobserve(video);
            });
        }, { rootMargin: '200px' });
        videos.forEach((video) => observer.observe(video));
    });
"""


def build_homepage():
    concerto_download_url = HERO_CONTENT["concerto_download_url"]
    install_display = INSTALL_CONTENT["command_display"].strip()
    install_copy = INSTALL_CONTENT["command_copy"]
    capability_items = CAPABILITIES_CONTENT["items"]
    building_blocks = BUILDING_BLOCKS_CONTENT["items"]

    concerto_product = PRODUCTS_CONTENT["concerto"]
    server_product = PRODUCTS_CONTENT["server"]
    server_terminal_lines = [
        (line["type"], line["content"]) for line in server_product["terminal_lines"]
    ]

    video_src = VIDEO_CONTENT.get("src") or None

    return (
        Title("Loopflow — Living software, conducted by you"),
        SkipLink(),
        Navbar(),
        Main(
            # Hero — tight: logo, headline, tagline, CTAs
            Section(
                Div(
                    Img(src="/static/logo.svg", alt="Loopflow", cls="hero-logo-large"),
                    H1("Loopflow"),
                    P(HERO_CONTENT["tagline"], cls="tagline"),
                    Div(
                        A("Download for Mac", href=concerto_download_url, cls="btn btn-primary"),
                        A("Read the docs", href="/docs", cls="btn btn-secondary"),
                        cls="btn-group hero-actions",
                    ),
                    cls="container hero-centered",
                ),
                cls="hero",
            ),
            # Demo visual — immediately after hero
            Section(
                Div(
                    DemoVideo(VIDEO_CONTENT["poster"], src=video_src),
                    cls="container",
                ),
                cls="hero-video-section",
            ),
            # Capabilities — 2-col grid
            Section(
                Div(
                    H2(CAPABILITIES_CONTENT["heading"]),
                    Div(
                        *[
                            CapabilityItem(item["title"], item["description"])
                            for item in capability_items
                        ],
                        cls="capabilities-grid",
                    ),
                ),
                cls="capabilities-section",
            ),
            # Building blocks — 2×2 grid
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
            # Products — 2-col grid
            Section(
                Div(
                    H2(PRODUCTS_CONTENT["heading"]),
                    Div(
                        ProductCard(
                            concerto_product["name"],
                            concerto_product["description"],
                            concerto_product["details"],
                            Img(
                                src=concerto_product["image"],
                                alt=concerto_product["image_alt"],
                            ),
                            note=concerto_product.get("note"),
                        ),
                        ProductCard(
                            server_product["name"],
                            server_product["description"],
                            server_product["details"],
                            TerminalBlock(server_terminal_lines),
                            note=server_product.get("note"),
                        ),
                        cls="two-products",
                    ),
                    cls="container",
                ),
                cls="products-section",
            ),
            # Bottom CTA — download + CLI install
            Section(
                Div(
                    Div(
                        A("Download for Mac", href=concerto_download_url, cls="btn btn-primary"),
                        A("Read the docs", href="/docs", cls="btn btn-secondary"),
                        cls="hero-actions",
                    ),
                    H2("Or install the CLI", cls="quick-install-heading"),
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
        Script(HOMEPAGE_VIDEO_BEHAVIOR_SCRIPT),
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
    # Redirect /products to home for now
    return RedirectResponse("/", status_code=302)


# Concerto runtime concepts (Triggers, Loops, Subscriptions)
CONCERTO_VOCAB_ESSENCES = {
    "Trigger": "What starts an agent",
    "Loop": "What keeps it going",
    "Subscription": "What connects them",
}

CONCERTO_VOCAB_DETAILS = {
    "Trigger": [
        ("Manual", "Button click or command"),
        ("File change", "Watch and respond"),
        ("Schedule", "Cron-style timing"),
    ],
    "Loop": [
        ("Goal-driven", "Runs until done"),
        ("Interruptible", "Pause, inspect, resume"),
    ],
    "Subscription": [
        ("Agent → Agent", "Output triggers input"),
        ("Broadcast", "One event, many agents"),
    ],
}

CONCERTO_VOCAB_ICONS = {
    "Trigger": _ICON_BOLT,
    "Loop": _ICON_LOOP,
    "Subscription": _ICON_SWAP,
}


@rt("/concerto")
def get():
    return (
        Title("Concerto — Loopflow UI"),
        SkipLink(),
        Navbar(),
        Main(
            # Hero
            Section(
                Div(
                    H1("Concerto"),
                    P("Arrange and conduct agent orchestras.", cls="tagline"),
                    cls="container hero-centered",
                ),
                cls="hero",
            ),
            # Runtime concepts
            Section(
                Div(
                    H2("The runtime"),
                    Div(
                        TermCard(
                            "1",
                            "Trigger",
                            CONCERTO_VOCAB_ICONS["Trigger"],
                            CONCERTO_VOCAB_ESSENCES["Trigger"],
                            CONCERTO_VOCAB_DETAILS["Trigger"],
                        ),
                        TermCard(
                            "2",
                            "Loop",
                            CONCERTO_VOCAB_ICONS["Loop"],
                            CONCERTO_VOCAB_ESSENCES["Loop"],
                            CONCERTO_VOCAB_DETAILS["Loop"],
                        ),
                        TermCard(
                            "3",
                            "Subscription",
                            CONCERTO_VOCAB_ICONS["Subscription"],
                            CONCERTO_VOCAB_ESSENCES["Subscription"],
                            CONCERTO_VOCAB_DETAILS["Subscription"],
                        ),
                        cls="vocab-grid vocab-grid-3",
                    ),
                    Script(VOCAB_ACCORDION_SCRIPT),
                    cls="container",
                ),
                cls="vocab-section",
            ),
            # Screenshot
            Section(
                Div(
                    Img(
                        src="/static/concerto-main.png",
                        alt="Concerto showing waves with active, idle, and scheduled states",
                        cls="concerto-showcase-img",
                    ),
                    P(
                        "All your agents, all your branches, one view.",
                        cls="concerto-showcase-caption",
                    ),
                    cls="container",
                ),
                cls="concerto-showcase-section",
            ),
            # Install
            Section(
                Div(
                    Div(
                        Pre(
                            Code("curl -fsSL https://loopflow.studio/install.sh | sh\nlf init"),
                            cls="install-code",
                            tabindex="0",
                        ),
                        CopyButton("curl -fsSL https://loopflow.studio/install.sh | sh && lf init"),
                        cls="install-code-wrapper",
                    ),
                    Div(
                        A("Install the CLI", href="/download", cls="btn btn-primary"),
                        A("Read the docs", href="/docs", cls="btn btn-secondary"),
                        cls="hero-actions",
                    ),
                    cls="container",
                ),
                cls="quick-install",
            ),
            id="main-content",
        ),
        SiteFooter(),
    )


@rt("/maestro")
def get():
    # Redirect /maestro to download
    return RedirectResponse("/download", status_code=302)


@rt("/symphonia")
def get():
    return (
        Title("Symphonia — Loopflow for Teams"),
        SkipLink(),
        Navbar(),
        Main(
            Section(
                Div(
                    Span(
                        "Coming Soon",
                        cls="maturity-badge maturity-experimental",
                        style="margin-bottom: 24px; display: inline-block;",
                    ),
                    H1("Symphonia"),
                    P("Concerto for teams.", cls="tagline"),
                    P(
                        "Server-side agents, coordinating agents at very large scale as a collective team.",
                        style="color: var(--text-secondary); font-size: 16px; max-width: 520px; margin: 0 auto 32px;",
                    ),
                    cls="container",
                ),
                cls="page-hero",
            ),
            Section(
                Div(
                    H2("Get in touch"),
                    P(
                        "Symphonia is in development. If you're interested in running coordinated agent teams at scale, we'd love to hear from you."
                    ),
                    A(
                        "teams@loopflow.studio",
                        href="mailto:teams@loopflow.studio",
                        cls="contact-email",
                    ),
                    cls="container",
                ),
                cls="contact-section",
            ),
            id="main-content",
        ),
        SiteFooter(),
    )


@rt("/team")
def get():
    return (
        Title("Team — Loopflow"),
        SkipLink(),
        Navbar(),
        Main(
            Section(
                Div(
                    H1("Team"),
                    P(
                        "I'm actively looking for co-founders. ",
                        A("Want to get in touch?", href="mailto:hello@loopflow.studio"),
                        cls="tagline",
                    ),
                    cls="container",
                ),
                cls="page-hero team-hero",
            ),
            Section(
                Div(
                    TeamCard(
                        name="Jack Heart",
                        title="Founder & CEO",
                        bio="Early engineer at Yelp, Asana, and Grail. Explored the emergence of higher-level agents out of collaboration at Softmax. Started Loopflow to combine my expertise in effective collaboration, my passion for simple APIs, and my joy in the creative process.",
                        image_src="/static/jack.jpg",
                    ),
                    cls="container",
                ),
                cls="team-section",
            ),
            id="main-content",
        ),
        SiteFooter(),
    )


@rt("/agents")
def get():
    # Redirect /agents to docs (agents is experimental and folded into Coming Soon)
    return RedirectResponse("/docs", status_code=302)


@rt("/docs")
def get():
    content = load_doc("index")
    return (
        Title("Loopflow Documentation"),
        SkipLink(),
        Navbar(),
        Main(
            Section(
                Div(
                    DocsNav("index"),
                    Div(*render_markdown(content), cls="docs-content"),
                    cls="docs-layout",
                ),
                cls="docs-hero",
            ),
            id="main-content",
        ),
        SiteFooter(),
    )


@rt("/docs/{slug:path}")
def get(slug: str):
    # Handle .md extension if present
    if slug.endswith(".md"):
        slug = slug[:-3]
    content = load_doc(slug)
    if not content:
        return RedirectResponse("/docs", status_code=302)
    # Get title from DOCS_NAV
    title = next((t for t, s in DOCS_NAV if s == slug), slug.title())
    return (
        Title(f"{title} — Loopflow Documentation"),
        SkipLink(),
        Navbar(),
        Main(
            Section(
                Div(
                    DocsNav(slug),
                    Div(*render_markdown(content), cls="docs-content"),
                    cls="docs-layout",
                ),
                cls="docs-hero",
            ),
            id="main-content",
        ),
        SiteFooter(),
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
                    P("Read the steps. Try the CLI. Decide with evidence.", cls="tagline"),
                    Div(
                        H2("CLI"),
                        P(
                            "Prompt and context construction from the terminal. Best for: daily work, CI, and careful pilots.",
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
                        P("macOS · Claude Code, Codex, or Gemini CLI", cls="system-req"),
                        Div(
                            P("Then:", cls="next-step-label"),
                            Pre(
                                Code("cd your-project\nlf init\nlf design"),
                                cls="install-code next-steps",
                                tabindex="0",
                            ),
                            cls="next-steps-wrapper",
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


# Insert llms.txt route at the beginning to avoid being captured by static handler
app.routes.insert(0, Route("/llms.txt", _llms_txt_handler, methods=["GET"]))


@rt("/favicon.ico")
async def favicon():
    """Serve logo.svg as favicon for browsers that request .ico."""
    return FileResponse("static/logo.svg", media_type="image/svg+xml")


@rt("/static/{fname:path}")
async def static(fname: str):
    return FileResponse(f"static/{fname}")


@rt("/waitlist")
async def post(email: str, product: str):
    """HTMX endpoint for waitlist signups."""
    if not email or not product:
        return P("Please provide an email.", style="color: var(--orange);")
    result = add_to_waitlist(email.strip().lower(), product)
    if result is None:
        return P("Something went wrong. Please try again.", style="color: var(--orange);")
    if result:
        return P("Thanks! We'll be in touch.", style="color: var(--green); font-weight: 500;")
    return P("You're already on the list.", style="color: var(--text-secondary);")


init_db()

if __name__ == "__main__":
    serve(host="0.0.0.0", port=int(os.environ.get("PORT", 5001)))
