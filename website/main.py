import os
import re
from pathlib import Path

import yaml
from fasthtml.common import *
from starlette.responses import PlainTextResponse, RedirectResponse
from starlette.routing import Route

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
                Li(A("Story", href="/story")),
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
                A("Story", href="/story"),
                A("GitHub", href="https://github.com/loopflowstudio/loopflow"),
                cls="footer-links",
            ),
            cls="container",
        ),
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
STORY_CONTENT = _require_content_path("homepage.story")
PILLARS_CONTENT = _require_content_path("homepage.pillars")
BUILDING_BLOCKS_CONTENT = _require_content_path("homepage.building_blocks")
TERMINAL_CONTENT = _require_content_path("homepage.terminal")
INSTALL_CONTENT = _require_content_path("homepage.install")

for required_key in (
    "homepage.hero.tagline",
    "homepage.hero.loopflow_download_url",
    "homepage.story.paragraphs",
    "homepage.story.link_href",
    "homepage.pillars.items",
    "homepage.building_blocks.items",
    "homepage.terminal.lines",
    "homepage.install.command_display",
    "homepage.install.command_copy",
):
    _require_content_path(required_key)

# Markdown rendering

DOCS_DIR = Path(__file__).parent / "docs"
CANONICAL_DOCS_DIR = Path(__file__).parent.parent / "docs"
STORY_FILE = Path(__file__).parent / "story.md"


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
    ("The Fleet", "fleet"),
    ("Architecture", "architecture"),
    # Reference
    ("lf", "lf"),
    ("Config", "config"),
    ("Troubleshooting", "troubleshooting"),
]


def generate_llms_txt() -> str:
    """Generate machine-readable doc summary for /llms.txt from the docs nav."""
    doc_lines = "\n".join(
        f"/docs/{slug}".ljust(24) + title if slug != "index" else "/docs".ljust(24) + title
        for title, slug in DOCS_NAV
    )
    return f"""# Loopflow
> Persistent agents, no server. Waves hold a goal, remember what they learn, and stay steerable.

## What is Loopflow?
Loopflow creates and runs Waves — persistent agents that work toward an outcome.
A Wave coordinates Linear-backed Projects and Tasks, keeps one steerable
conversation beside the live work map, and folds what it learns into memory.
Everything is one binary: lf is the command humans type and the API agents call
to launch, steer, and observe other agents. There is no server — state lives in
a local SQLite store and append-only journals, shared truth lives in Linear and
GitHub, and remote machines are reached over SSH.

## Install
curl -fsSL https://loopflow.studio/install.sh | sh && lf init

## Commands
lf <skill>            Run a skill (design, implement, gate, debug, ...)
lf debug -c           Fix an error from the clipboard
lf wave X             Start Wave X's resident process
lf chat --steer "..." Steer the live wave body (humans)
lf radio pub/sub      Agent-to-agent bus; publish is an INSERT, no broker
lf task run ID        Start a durable Task Session from a Linear issue
lf task steer ID ".." Redirect a running Task; receipts prove incorporation
lf roadmap            Every open Task across every wave, bucketed by need
lf status <wave>      One wave's live Project → Task hierarchy
lf trace / lf context What an agent did, and exactly what it was told
Every read surface takes --json.

## Core Concepts
Wave: a named agent with a goal — wave/<name>/GOAL.md (intent + loop prompt)
  and wave/<name>/MEMORY.md (durable memory the wave writes).
Project: one measured Linear-backed bet under a Wave; pursues KRs, owns no worktree.
Task: one concrete Linear issue; its Session owns the only delivery worktree
  and advances through serial PRs to main.
Home: where a wave's work executes — owner plus location, local or ssh://.
Skill: a prompt that runs a coding agent. Flow: a sequence of skills.
Direction: composable quality intents that shape agent judgment.

## Docs
{doc_lines}

## Links
GitHub: https://github.com/loopflowstudio/loopflow
Docs: https://loopflow.studio/docs
Story: https://loopflow.studio/story
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


def load_doc(slug: str) -> str:
    for docs_dir in (DOCS_DIR, CANONICAL_DOCS_DIR):
        path = docs_dir / f"{slug}.md"
        if path.exists():
            return path.read_text()
    return ""


# Pages


def build_homepage():
    loopflow_download_url = HERO_CONTENT["loopflow_download_url"]
    install_display = INSTALL_CONTENT["command_display"].strip()
    install_copy = INSTALL_CONTENT["command_copy"]
    story_paragraphs = STORY_CONTENT["paragraphs"]
    pillar_items = PILLARS_CONTENT["items"]
    building_blocks = BUILDING_BLOCKS_CONTENT["items"]
    terminal_lines = [(line["type"], line["content"]) for line in TERMINAL_CONTENT["lines"]]

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
                    Div(
                        A("Install", href="/download", cls="btn btn-primary"),
                        A("Read the docs", href="/docs", cls="btn btn-secondary"),
                        cls="btn-group hero-actions",
                    ),
                    cls="container hero-centered",
                ),
                cls="hero",
            ),
            # The story — the narrative leads
            Section(
                Div(
                    H2(STORY_CONTENT["heading"]),
                    *[P(p, cls="story-paragraph") for p in story_paragraphs],
                    P(
                        A(
                            STORY_CONTENT["link_label"] + " →",
                            href=STORY_CONTENT["link_href"],
                        ),
                        cls="story-link",
                    ),
                    cls="container story-container",
                ),
                cls="story-section",
            ),
            # Pillars — what the pressure produced
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
            # Terminal — watch it work
            Section(
                Div(
                    H2(TERMINAL_CONTENT["heading"]),
                    TerminalBlock(terminal_lines),
                    cls="container terminal-showcase",
                ),
                cls="terminal-section",
            ),
            # Bottom CTA — CLI install + Mac app
            Section(
                Div(
                    H2("Install the CLI", cls="quick-install-heading"),
                    Div(
                        Pre(Code(install_display), cls="install-code", tabindex="0"),
                        CopyButton(install_copy),
                        cls="install-code-wrapper",
                    ),
                    Div(
                        A("Download for Mac", href=loopflow_download_url, cls="btn btn-secondary"),
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
    # The team page became the story page
    return RedirectResponse("/story", status_code=302)


@rt("/story")
def get():
    content = STORY_FILE.read_text()
    return (
        Title("The Story — Loopflow"),
        SkipLink(),
        Navbar(),
        Main(
            Section(
                Div(
                    Div(*render_markdown(content), cls="docs-content story-page"),
                    cls="container",
                ),
                cls="docs-hero",
            ),
            id="main-content",
        ),
        SiteFooter(),
    )


@rt("/agents")
def get():
    # Redirect /agents to docs
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
                                "The fleet cockpit: wave chat, the machine-wide roadmap, and every task's worktree — a pure client over the same local state.",
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


# Insert llms.txt route at the beginning to avoid being captured by static handler
app.routes.insert(0, Route("/llms.txt", _llms_txt_handler, methods=["GET"]))


@rt("/favicon.ico")
async def favicon():
    """Serve logo.svg as favicon for browsers that request .ico."""
    return FileResponse("static/logo.svg", media_type="image/svg+xml")


@rt("/static/{fname:path}")
async def static(fname: str):
    return FileResponse(f"static/{fname}")


if __name__ == "__main__":
    serve(host="0.0.0.0", port=int(os.environ.get("PORT", 5001)))
