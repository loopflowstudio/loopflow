"""Internal dev tool pages: /fonts, /colors, /design.

Not linked from the public site. Used for design system reference.
"""

from fasthtml.common import *


def fonts_page():
    """Font comparison page for testing serif and sans-serif options."""
    return Html(
        Head(
            Title("Font Comparison | Loopflow"),
            Link(rel="stylesheet", href="https://fonts.googleapis.com/css2?family=Cormorant+Garamond:ital,wght@0,400;0,600;1,400;1,600&family=EB+Garamond:ital,wght@0,400;0,600;1,400;1,600&family=Playfair+Display:ital,wght@0,400;0,600;1,400;1,600&family=Fraunces:ital,opsz,wght@0,9..144,400;0,9..144,600;1,9..144,400;1,9..144,600&family=Lato:wght@400;500;700&family=Source+Sans+3:wght@400;500;600&family=Nunito+Sans:wght@400;500;600&family=Open+Sans:wght@400;500;600&family=IBM+Plex+Sans:wght@400;500;600&family=JetBrains+Mono:wght@400;500;600&family=Fira+Code:wght@400;500;600&family=Source+Code+Pro:wght@400;500;600&family=IBM+Plex+Mono:wght@400;500;600&display=swap"),
            Style("""
                :root {
                    --bg: #FAF8F5;
                    --text: #1A1A1A;
                    --text-secondary: #6B6B6B;
                    --border: #E3DDD5;
                    --burgundy: #722F37;
                }
                body {
                    font-family: -apple-system, BlinkMacSystemFont, sans-serif;
                    background: var(--bg);
                    color: var(--text);
                    padding: 48px 24px;
                    max-width: 1200px;
                    margin: 0 auto;
                }
                h1 {
                    font-size: 24px;
                    font-weight: 600;
                    margin-bottom: 8px;
                }
                h2 {
                    font-size: 20px;
                    font-weight: 600;
                    margin: 64px 0 24px;
                    padding-top: 32px;
                    border-top: 2px solid var(--border);
                }
                .subtitle {
                    color: var(--text-secondary);
                    margin-bottom: 48px;
                }
                .font-grid {
                    display: grid;
                    gap: 32px;
                }
                .font-card {
                    border: 1px solid var(--border);
                    border-radius: 12px;
                    padding: 32px;
                    background: white;
                }
                .font-card.chosen {
                    border-color: var(--burgundy);
                    border-width: 2px;
                }
                .font-name {
                    font-size: 14px;
                    font-weight: 600;
                    text-transform: uppercase;
                    letter-spacing: 0.05em;
                    color: var(--text-secondary);
                    margin-bottom: 24px;
                }
                .font-name .badge {
                    background: var(--burgundy);
                    color: white;
                    padding: 2px 8px;
                    border-radius: 4px;
                    font-size: 11px;
                    margin-left: 8px;
                    text-transform: uppercase;
                }
                .sample-row {
                    display: flex;
                    gap: 48px;
                    align-items: baseline;
                    margin-bottom: 16px;
                }
                .sample-label {
                    font-size: 11px;
                    color: var(--text-secondary);
                    margin-bottom: 4px;
                    text-transform: uppercase;
                    letter-spacing: 0.05em;
                }
                .sample-text {
                    font-size: 48px;
                    line-height: 1.2;
                }
                .sample-text.italic {
                    font-style: italic;
                }
                .sample-body {
                    font-size: 16px;
                    line-height: 1.6;
                    color: var(--text-secondary);
                    margin-top: 16px;
                    max-width: 600px;
                }
                .sample-button {
                    display: inline-block;
                    margin-top: 16px;
                    padding: 12px 24px;
                    background: var(--burgundy);
                    color: white;
                    border-radius: 6px;
                    font-size: 15px;
                    font-weight: 500;
                }
                .notes {
                    font-size: 14px;
                    color: var(--text-secondary);
                    margin-top: 24px;
                    padding-top: 24px;
                    border-top: 1px solid var(--border);
                }

                /* Serif families */
                .font-cormorant-garamond .sample-text,
                .font-cormorant-garamond .sample-body { font-family: var(--font-serif); }

                .font-eb-garamond .sample-text,
                .font-eb-garamond .sample-body { font-family: 'EB Garamond', Georgia, serif; }

                .font-playfair .sample-text,
                .font-playfair .sample-body { font-family: 'Playfair Display', Georgia, serif; }

                .font-fraunces .sample-text,
                .font-fraunces .sample-body { font-family: 'Fraunces', Georgia, serif; }

                /* Sans-serif families */
                .font-lato .sample-text,
                .font-lato .sample-body,
                .font-lato .sample-button { font-family: 'Lato', sans-serif; }

                .font-source-sans .sample-text,
                .font-source-sans .sample-body,
                .font-source-sans .sample-button { font-family: 'Source Sans 3', sans-serif; }

                .font-nunito-sans .sample-text,
                .font-nunito-sans .sample-body,
                .font-nunito-sans .sample-button { font-family: 'Nunito Sans', sans-serif; }

                .font-open-sans .sample-text,
                .font-open-sans .sample-body,
                .font-open-sans .sample-button { font-family: 'Open Sans', sans-serif; }

                .font-ibm-plex .sample-text,
                .font-ibm-plex .sample-body,
                .font-ibm-plex .sample-button { font-family: 'IBM Plex Sans', sans-serif; }

                /* Monospace families */
                .font-jetbrains .sample-text,
                .font-jetbrains .sample-code { font-family: 'JetBrains Mono', monospace; }

                .font-fira-code .sample-text,
                .font-fira-code .sample-code { font-family: 'Fira Code', monospace; }

                .font-source-code .sample-text,
                .font-source-code .sample-code { font-family: 'Source Code Pro', monospace; }

                .font-ibm-plex-mono .sample-text,
                .font-ibm-plex-mono .sample-code { font-family: 'IBM Plex Mono', monospace; }

                .sample-code {
                    font-size: 14px;
                    line-height: 1.5;
                    background: #1a1a1a;
                    color: #e0e0e0;
                    padding: 16px 20px;
                    border-radius: 8px;
                    margin-top: 16px;
                    white-space: pre;
                    overflow-x: auto;
                }
            """),
        ),
        Body(
            H1("Font Comparison"),
            P("Current selections and alternatives", cls="subtitle"),

            # Current selections
            Div(
                # Cormorant Garamond (chosen serif)
                Div(
                    Div(
                        Span("Cormorant Garamond"),
                        Span("serif", cls="badge"),
                        cls="font-name",
                    ),
                    Div(
                        Div("Loopflow", cls="sample-text"),
                        Div("Loopflow", cls="sample-text italic"),
                        cls="sample-row",
                    ),
                    P("Agents that remember. Work that compounds.", cls="sample-body"),
                    P("Headlines and taglines. Italic reserved for special moments.", cls="notes"),
                    cls="font-card font-cormorant-garamond chosen",
                ),
                # Lato (chosen sans-serif)
                Div(
                    Div(
                        Span("Lato"),
                        Span("sans-serif", cls="badge"),
                        cls="font-name",
                    ),
                    Div("Agents that remember", cls="sample-text"),
                    P("Context that travels with your agents, prompts worth keeping, and tight feedback loops.", cls="sample-body"),
                    Div("Get Started", cls="sample-button"),
                    P("Body text, buttons, UI. Warm humanist that pairs naturally with Garamond.", cls="notes"),
                    cls="font-card font-lato chosen",
                ),
                # JetBrains Mono (chosen monospace)
                Div(
                    Div(
                        Span("JetBrains Mono"),
                        Span("monospace", cls="badge"),
                        cls="font-name",
                    ),
                    Div("lf design → implement", cls="sample-text"),
                    Pre("$ lf init\n$ lf design\nDesigning on branch: feature-auth\n$ lf implement", cls="sample-code"),
                    P("Code, terminal, technical content. Clear character distinction.", cls="notes"),
                    cls="font-card font-jetbrains chosen",
                ),
                cls="font-grid",
            ),

            H2("Alternative Serifs"),
            Div(
                # EB Garamond
                Div(
                    Div("EB Garamond", cls="font-name"),
                    Div(
                        Div("Loopflow", cls="sample-text"),
                        Div("Loopflow", cls="sample-text italic"),
                        cls="sample-row",
                    ),
                    P("Agents that remember. Work that compounds.", cls="sample-body"),
                    P("Historical revival of Claude Garamont's types. Includes the long ſ character.", cls="notes"),
                    cls="font-card font-eb-garamond",
                ),
                # Playfair Display
                Div(
                    Div("Playfair Display", cls="font-name"),
                    Div(
                        Div("Loopflow", cls="sample-text"),
                        Div("Loopflow", cls="sample-text italic"),
                        cls="sample-row",
                    ),
                    P("Agents that remember. Work that compounds.", cls="sample-body"),
                    P("High-contrast transitional serif. Decorative italics from the pointed steel pen era.", cls="notes"),
                    cls="font-card font-playfair",
                ),
                # Fraunces
                Div(
                    Div("Fraunces", cls="font-name"),
                    Div(
                        Div("Loopflow", cls="sample-text"),
                        Div("Loopflow", cls="sample-text italic"),
                        cls="sample-row",
                    ),
                    P("Agents that remember. Work that compounds.", cls="sample-body"),
                    P("Playful old-style with Art Nouveau italic influence. Previous choice.", cls="notes"),
                    cls="font-card font-fraunces",
                ),
                cls="font-grid",
            ),

            H2("Alternative Sans-Serifs"),
            Div(
                # Source Sans 3
                Div(
                    Div("Source Sans 3", cls="font-name"),
                    Div("Agents that remember", cls="sample-text"),
                    P("Context that travels with your agents, prompts worth keeping, and tight feedback loops.", cls="sample-body"),
                    Div("Get Started", cls="sample-button"),
                    P("Adobe's first open-source font. Clean humanist with excellent readability.", cls="notes"),
                    cls="font-card font-source-sans",
                ),
                # Nunito Sans
                Div(
                    Div("Nunito Sans", cls="font-name"),
                    Div("Agents that remember", cls="sample-text"),
                    P("Context that travels with your agents, prompts worth keeping, and tight feedback loops.", cls="sample-body"),
                    Div("Get Started", cls="sample-button"),
                    P("Rounded terminals give it a softer, friendlier feel.", cls="notes"),
                    cls="font-card font-nunito-sans",
                ),
                # Open Sans
                Div(
                    Div("Open Sans", cls="font-name"),
                    Div("Agents that remember", cls="sample-text"),
                    P("Context that travels with your agents, prompts worth keeping, and tight feedback loops.", cls="sample-body"),
                    Div("Get Started", cls="sample-button"),
                    P("Classic humanist workhorse. Neutral but warm.", cls="notes"),
                    cls="font-card font-open-sans",
                ),
                # IBM Plex Sans
                Div(
                    Div("IBM Plex Sans", cls="font-name"),
                    Div("Agents that remember", cls="sample-text"),
                    P("Context that travels with your agents, prompts worth keeping, and tight feedback loops.", cls="sample-body"),
                    Div("Get Started", cls="sample-button"),
                    P("More technical/corporate feel. Previous choice.", cls="notes"),
                    cls="font-card font-ibm-plex",
                ),
                cls="font-grid",
            ),

            H2("Alternative Monospace"),
            Div(
                # Fira Code
                Div(
                    Div("Fira Code", cls="font-name"),
                    Div("lf design → implement", cls="sample-text"),
                    Pre("$ lf init\n$ lf design\nDesigning on branch: feature-auth\n$ lf implement", cls="sample-code"),
                    P("Popular with ligatures. Mozilla origin, open source.", cls="notes"),
                    cls="font-card font-fira-code",
                ),
                # Source Code Pro
                Div(
                    Div("Source Code Pro", cls="font-name"),
                    Div("lf design → implement", cls="sample-text"),
                    Pre("$ lf init\n$ lf design\nDesigning on branch: feature-auth\n$ lf implement", cls="sample-code"),
                    P("Adobe's monospace. Clean, neutral, highly legible.", cls="notes"),
                    cls="font-card font-source-code",
                ),
                # IBM Plex Mono
                Div(
                    Div("IBM Plex Mono", cls="font-name"),
                    Div("lf design → implement", cls="sample-text"),
                    Pre("$ lf init\n$ lf design\nDesigning on branch: feature-auth\n$ lf implement", cls="sample-code"),
                    P("Pairs with IBM Plex Sans. Distinctive character.", cls="notes"),
                    cls="font-card font-ibm-plex-mono",
                ),
                cls="font-grid",
            ),
        ),
    )


def colors_page():
    """Color palette showcase page."""
    return Html(
        Head(
            Title("Colors | Loopflow Design System"),
            Link(rel="stylesheet", href="https://fonts.googleapis.com/css2?family=Cormorant+Garamond:wght@400;600&family=Lato:wght@400;700&family=JetBrains+Mono:wght@400&display=swap"),
            Style("""
                :root {
                    --burgundy: #722F37;
                    --burgundy-hover: #8B3D47;
                    --bg: #FAF8F5;
                    --bg-surface: #FFFDFB;
                    --bg-muted: #F3EEE7;
                    --border: #E3DDD5;
                    --text: #1A1A1A;
                    --text-secondary: #6B6B6B;
                    --success: #2D6A4F;
                    --warning: #B0812A;
                    --error: #B45309;
                    --info: #0AB3CC;
                    --font-serif: 'Cormorant Garamond', Georgia, serif;
                    --font-sans: 'Lato', -apple-system, sans-serif;
                    --font-mono: 'JetBrains Mono', monospace;
                }
                * { box-sizing: border-box; margin: 0; padding: 0; }
                body {
                    font-family: var(--font-sans);
                    background: var(--bg);
                    color: var(--text);
                    padding: 48px 24px;
                    max-width: 1100px;
                    margin: 0 auto;
                    line-height: 1.6;
                }
                h1 {
                    font-family: var(--font-serif);
                    font-size: 48px;
                    font-weight: 400;
                    margin-bottom: 8px;
                    color: var(--burgundy);
                }
                .subtitle {
                    font-size: 18px;
                    color: var(--text-secondary);
                    margin-bottom: 48px;
                }
                h2 {
                    font-size: 24px;
                    font-weight: 600;
                    margin: 56px 0 24px;
                    padding-top: 32px;
                    border-top: 1px solid var(--border);
                }
                h2:first-of-type { margin-top: 0; padding-top: 0; border-top: none; }
                .color-grid {
                    display: grid;
                    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
                    gap: 24px;
                }
                .color-card {
                    border: 1px solid var(--border);
                    border-radius: 12px;
                    overflow: hidden;
                    background: white;
                }
                .color-swatch {
                    height: 120px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    font-family: var(--font-mono);
                    font-size: 14px;
                }
                .color-info {
                    padding: 16px;
                    border-top: 1px solid var(--border);
                }
                .color-name {
                    font-weight: 600;
                    margin-bottom: 4px;
                }
                .color-token {
                    font-family: var(--font-mono);
                    font-size: 13px;
                    color: var(--text-secondary);
                }
                .color-usage {
                    font-size: 14px;
                    color: var(--text-secondary);
                    margin-top: 8px;
                }
                .logo-section {
                    display: flex;
                    align-items: center;
                    gap: 32px;
                    padding: 32px;
                    background: var(--bg-muted);
                    border-radius: 12px;
                    margin-bottom: 32px;
                }
                .logo-section img {
                    width: 80px;
                    height: 80px;
                }
                .logo-colors {
                    display: flex;
                    gap: 16px;
                }
                .logo-color {
                    text-align: center;
                }
                .logo-swatch {
                    width: 60px;
                    height: 60px;
                    border-radius: 8px;
                    margin-bottom: 8px;
                }
                .logo-hex {
                    font-family: var(--font-mono);
                    font-size: 12px;
                    color: var(--text-secondary);
                }
                .relationship {
                    background: var(--bg-surface);
                    border: 1px solid var(--border);
                    border-radius: 12px;
                    padding: 24px;
                    margin: 24px 0;
                }
                .relationship pre {
                    font-family: var(--font-mono);
                    font-size: 14px;
                    line-height: 1.8;
                    color: var(--text-secondary);
                }
                .nav-back {
                    display: inline-flex;
                    align-items: center;
                    gap: 8px;
                    color: var(--burgundy);
                    text-decoration: none;
                    font-size: 14px;
                    margin-bottom: 32px;
                }
                .nav-back:hover { text-decoration: underline; }
                .dark-preview {
                    background: #2B3036;
                    padding: 32px;
                    border-radius: 12px;
                    margin-top: 24px;
                }
                .dark-preview .color-grid { gap: 16px; }
                .dark-preview .color-card {
                    background: #343B44;
                    border-color: #46505B;
                }
                .dark-preview .color-info {
                    border-color: #46505B;
                    color: #F5F1EA;
                }
                .dark-preview .color-token,
                .dark-preview .color-usage { color: #C8C1B8; }
            """),
        ),
        Body(
            A("← Back to Home", href="/", cls="nav-back"),
            H1("Color Palette"),
            P("Unified colors for loopflow and loopflowstudio — deep burgundy meets warm cream.", cls="subtitle"),

            H2("Brand Foundation"),
            Div(
                Img(src="/static/logo.svg", alt="Loopflow Logo"),
                Div(
                    P("The logo uses a gradient from wine to cyan:", style="margin-bottom: 12px; color: var(--text-secondary);"),
                    Div(
                        Div(
                            Div(style="background: #9B1A4A;", cls="logo-swatch"),
                            Div("#9B1A4A", cls="logo-hex"),
                            Div("Wine", style="font-size: 12px;"),
                            cls="logo-color",
                        ),
                        Div(
                            Div(style="background: linear-gradient(135deg, #9B1A4A, #0AB3CC);", cls="logo-swatch"),
                            Div("Gradient", cls="logo-hex"),
                            Div("Logo", style="font-size: 12px;"),
                            cls="logo-color",
                        ),
                        Div(
                            Div(style="background: #0AB3CC;", cls="logo-swatch"),
                            Div("#0AB3CC", cls="logo-hex"),
                            Div("Cyan", style="font-size: 12px;"),
                            cls="logo-color",
                        ),
                        cls="logo-colors",
                    ),
                ),
                cls="logo-section",
            ),

            Div(
                Pre("""Logo Wine (#9B1A4A)
        ↓
        ↓  Darkened for UI use
        ↓
Primary Burgundy (#722F37) ← UI accent color
        ↓
        ↓  Lightened for hover
        ↓
Burgundy Hover (#8B3D47)"""),
                cls="relationship",
            ),
            P("The burgundy is the logo wine's 'indoor voice' — same family, but appropriate for sustained UI use.", style="color: var(--text-secondary); margin-bottom: 32px;"),

            H2("Primary Accent"),
            Div(
                Div(
                    Div(style="background: #722F37; color: white;", cls="color-swatch"),
                    Div(
                        Div("Burgundy", cls="color-name"),
                        Div("--burgundy", cls="color-token"),
                        Div("#722F37 · rgb(114, 47, 55)", cls="color-token"),
                        P("CTAs, buttons, links, focus states", cls="color-usage"),
                        cls="color-info",
                    ),
                    cls="color-card",
                ),
                Div(
                    Div(style="background: #8B3D47; color: white;", cls="color-swatch"),
                    Div(
                        Div("Burgundy Hover", cls="color-name"),
                        Div("--burgundy-hover", cls="color-token"),
                        Div("#8B3D47 · rgb(139, 61, 71)", cls="color-token"),
                        P("Hover and active states", cls="color-usage"),
                        cls="color-info",
                    ),
                    cls="color-card",
                ),
                cls="color-grid",
            ),

            H2("Light Mode (Cream)"),
            Div(
                Div(
                    Div(style="background: #FAF8F5; border: 1px solid var(--border);", cls="color-swatch"),
                    Div(
                        Div("Background", cls="color-name"),
                        Div("--bg", cls="color-token"),
                        Div("#FAF8F5", cls="color-token"),
                        P("Main page background", cls="color-usage"),
                        cls="color-info",
                    ),
                    cls="color-card",
                ),
                Div(
                    Div(style="background: #FFFDFB; border: 1px solid var(--border);", cls="color-swatch"),
                    Div(
                        Div("Surface", cls="color-name"),
                        Div("--bg-surface", cls="color-token"),
                        Div("#FFFDFB", cls="color-token"),
                        P("Cards, modals, elevated elements", cls="color-usage"),
                        cls="color-info",
                    ),
                    cls="color-card",
                ),
                Div(
                    Div(style="background: #F3EEE7;", cls="color-swatch"),
                    Div(
                        Div("Muted", cls="color-name"),
                        Div("--bg-muted", cls="color-token"),
                        Div("#F3EEE7", cls="color-token"),
                        P("Secondary surfaces, code blocks", cls="color-usage"),
                        cls="color-info",
                    ),
                    cls="color-card",
                ),
                Div(
                    Div(style="background: #E3DDD5;", cls="color-swatch"),
                    Div(
                        Div("Border", cls="color-name"),
                        Div("--border", cls="color-token"),
                        Div("#E3DDD5", cls="color-token"),
                        P("Borders, dividers", cls="color-usage"),
                        cls="color-info",
                    ),
                    cls="color-card",
                ),
                Div(
                    Div(style="background: #1A1A1A; color: white;", cls="color-swatch"),
                    Div(
                        Div("Text", cls="color-name"),
                        Div("--text", cls="color-token"),
                        Div("#1A1A1A", cls="color-token"),
                        P("Primary text", cls="color-usage"),
                        cls="color-info",
                    ),
                    cls="color-card",
                ),
                Div(
                    Div(style="background: #6B6B6B; color: white;", cls="color-swatch"),
                    Div(
                        Div("Text Secondary", cls="color-name"),
                        Div("--text-secondary", cls="color-token"),
                        Div("#6B6B6B", cls="color-token"),
                        P("Captions, muted text", cls="color-usage"),
                        cls="color-info",
                    ),
                    cls="color-card",
                ),
                cls="color-grid",
            ),

            H2("Dark Mode (Slate)"),
            Div(
                Div(
                    Div(
                        Div(style="background: #2B3036; color: #F5F1EA;", cls="color-swatch"),
                        Div(
                            Div("Background", cls="color-name"),
                            Div("--bg", cls="color-token"),
                            Div("#2B3036", cls="color-token"),
                            cls="color-info",
                        ),
                        cls="color-card",
                    ),
                    Div(
                        Div(style="background: #343B44; color: #F5F1EA;", cls="color-swatch"),
                        Div(
                            Div("Surface", cls="color-name"),
                            Div("--bg-surface", cls="color-token"),
                            Div("#343B44", cls="color-token"),
                            cls="color-info",
                        ),
                        cls="color-card",
                    ),
                    Div(
                        Div(style="background: #3C4550; color: #F5F1EA;", cls="color-swatch"),
                        Div(
                            Div("Muted", cls="color-name"),
                            Div("--bg-muted", cls="color-token"),
                            Div("#3C4550", cls="color-token"),
                            cls="color-info",
                        ),
                        cls="color-card",
                    ),
                    Div(
                        Div(style="background: #46505B; color: #F5F1EA;", cls="color-swatch"),
                        Div(
                            Div("Border", cls="color-name"),
                            Div("--border", cls="color-token"),
                            Div("#46505B", cls="color-token"),
                            cls="color-info",
                        ),
                        cls="color-card",
                    ),
                    Div(
                        Div(style="background: #F5F1EA; color: #2B3036;", cls="color-swatch"),
                        Div(
                            Div("Text", cls="color-name"),
                            Div("--text", cls="color-token"),
                            Div("#F5F1EA", cls="color-token"),
                            cls="color-info",
                        ),
                        cls="color-card",
                    ),
                    Div(
                        Div(style="background: #C8C1B8; color: #2B3036;", cls="color-swatch"),
                        Div(
                            Div("Text Secondary", cls="color-name"),
                            Div("--text-secondary", cls="color-token"),
                            Div("#C8C1B8", cls="color-token"),
                            cls="color-info",
                        ),
                        cls="color-card",
                    ),
                    cls="color-grid",
                ),
                cls="dark-preview",
            ),

            H2("Status Colors"),
            Div(
                Div(
                    Div(style="background: #2D6A4F; color: white;", cls="color-swatch"),
                    Div(
                        Div("Success", cls="color-name"),
                        Div("--success", cls="color-token"),
                        Div("#2D6A4F", cls="color-token"),
                        P("Stable, complete, passing", cls="color-usage"),
                        cls="color-info",
                    ),
                    cls="color-card",
                ),
                Div(
                    Div(style="background: #B0812A; color: white;", cls="color-swatch"),
                    Div(
                        Div("Warning", cls="color-name"),
                        Div("--warning", cls="color-token"),
                        Div("#B0812A", cls="color-token"),
                        P("Early-stage, caution", cls="color-usage"),
                        cls="color-info",
                    ),
                    cls="color-card",
                ),
                Div(
                    Div(style="background: #B45309; color: white;", cls="color-swatch"),
                    Div(
                        Div("Error", cls="color-name"),
                        Div("--error", cls="color-token"),
                        Div("#B45309", cls="color-token"),
                        P("Experimental, failing", cls="color-usage"),
                        cls="color-info",
                    ),
                    cls="color-card",
                ),
                Div(
                    Div(style="background: #0AB3CC; color: white;", cls="color-swatch"),
                    Div(
                        Div("Info", cls="color-name"),
                        Div("--info", cls="color-token"),
                        Div("#0AB3CC", cls="color-token"),
                        P("Informational", cls="color-usage"),
                        cls="color-info",
                    ),
                    cls="color-card",
                ),
                cls="color-grid",
            ),
        ),
    )


def design_page():
    """Design system overview page."""
    return Html(
        Head(
            Title("Design System | Loopflow"),
            Link(rel="stylesheet", href="https://fonts.googleapis.com/css2?family=Cormorant+Garamond:ital,wght@0,400;0,600;1,400&family=Lato:wght@400;700;900&family=JetBrains+Mono:wght@400;500&display=swap"),
            Style("""
                :root {
                    --burgundy: #722F37;
                    --burgundy-hover: #8B3D47;
                    --bg: #FAF8F5;
                    --bg-surface: #FFFDFB;
                    --bg-muted: #F3EEE7;
                    --border: #E3DDD5;
                    --text: #1A1A1A;
                    --text-secondary: #6B6B6B;
                    --font-serif: 'Cormorant Garamond', Georgia, serif;
                    --font-sans: 'Lato', -apple-system, sans-serif;
                    --font-mono: 'JetBrains Mono', monospace;
                }
                * { box-sizing: border-box; margin: 0; padding: 0; }
                body {
                    font-family: var(--font-sans);
                    background: var(--bg);
                    color: var(--text);
                    padding: 48px 24px;
                    max-width: 1100px;
                    margin: 0 auto;
                    line-height: 1.6;
                }
                h1 {
                    font-family: var(--font-serif);
                    font-size: 48px;
                    font-weight: 400;
                    margin-bottom: 8px;
                    color: var(--burgundy);
                }
                .subtitle {
                    font-size: 18px;
                    color: var(--text-secondary);
                    margin-bottom: 48px;
                }
                h2 {
                    font-size: 24px;
                    font-weight: 600;
                    margin: 56px 0 24px;
                    padding-top: 32px;
                    border-top: 1px solid var(--border);
                }
                h2:first-of-type { margin-top: 0; padding-top: 0; border-top: none; }
                h3 {
                    font-size: 18px;
                    font-weight: 600;
                    margin: 32px 0 16px;
                }
                p { margin-bottom: 16px; }
                .nav-back {
                    display: inline-flex;
                    align-items: center;
                    gap: 8px;
                    color: var(--burgundy);
                    text-decoration: none;
                    font-size: 14px;
                    margin-bottom: 32px;
                }
                .nav-back:hover { text-decoration: underline; }
                .nav-links {
                    display: flex;
                    gap: 24px;
                    margin-bottom: 48px;
                }
                .nav-links a {
                    color: var(--burgundy);
                    text-decoration: none;
                    padding: 8px 16px;
                    border: 1px solid var(--burgundy);
                    border-radius: 6px;
                    font-size: 14px;
                }
                .nav-links a:hover {
                    background: var(--burgundy);
                    color: white;
                }
                .type-sample {
                    background: var(--bg-surface);
                    border: 1px solid var(--border);
                    border-radius: 12px;
                    padding: 32px;
                    margin-bottom: 24px;
                }
                .type-label {
                    font-size: 12px;
                    text-transform: uppercase;
                    letter-spacing: 0.1em;
                    color: var(--text-secondary);
                    margin-bottom: 12px;
                }
                .type-serif {
                    font-family: var(--font-serif);
                    font-size: 36px;
                    line-height: 1.3;
                    margin-bottom: 8px;
                }
                .type-serif-italic {
                    font-family: var(--font-serif);
                    font-style: italic;
                    font-size: 24px;
                    color: var(--text-secondary);
                }
                .type-sans {
                    font-family: var(--font-sans);
                    font-size: 16px;
                    line-height: 1.6;
                    max-width: 600px;
                }
                .type-mono {
                    font-family: var(--font-mono);
                    font-size: 14px;
                    background: #1e1e1e;
                    color: #e0e0e0;
                    padding: 20px 24px;
                    border-radius: 8px;
                    overflow-x: auto;
                }
                .type-mono .prompt { color: #27ca40; }
                .type-mono .comment { color: #6a9955; }
                .spacing-grid {
                    display: grid;
                    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
                    gap: 16px;
                }
                .spacing-item {
                    text-align: center;
                }
                .spacing-box {
                    background: var(--burgundy);
                    margin: 0 auto 8px;
                    opacity: 0.7;
                }
                .spacing-label {
                    font-family: var(--font-mono);
                    font-size: 13px;
                    color: var(--text-secondary);
                }
                .spacing-value {
                    font-size: 12px;
                    color: var(--text-secondary);
                }
                .button-grid {
                    display: flex;
                    gap: 16px;
                    flex-wrap: wrap;
                    align-items: center;
                }
                .btn {
                    display: inline-flex;
                    align-items: center;
                    justify-content: center;
                    padding: 12px 24px;
                    border-radius: 8px;
                    font-family: var(--font-sans);
                    font-size: 15px;
                    font-weight: 500;
                    text-decoration: none;
                    cursor: pointer;
                    border: none;
                    transition: all 0.15s ease;
                }
                .btn-primary {
                    background: var(--burgundy);
                    color: white;
                }
                .btn-primary:hover { background: var(--burgundy-hover); }
                .btn-secondary {
                    background: transparent;
                    color: var(--text);
                    border: 1px solid var(--border);
                }
                .btn-secondary:hover { border-color: var(--text-secondary); }
                .radius-grid {
                    display: flex;
                    gap: 24px;
                    flex-wrap: wrap;
                }
                .radius-item {
                    text-align: center;
                }
                .radius-box {
                    width: 80px;
                    height: 80px;
                    background: var(--bg-muted);
                    border: 2px solid var(--burgundy);
                    margin-bottom: 8px;
                }
                .radius-label {
                    font-family: var(--font-mono);
                    font-size: 12px;
                    color: var(--text-secondary);
                }
                .principle-card {
                    background: var(--bg-surface);
                    border: 1px solid var(--border);
                    border-radius: 12px;
                    padding: 24px;
                    margin-bottom: 16px;
                }
                .principle-card h4 {
                    font-size: 16px;
                    font-weight: 600;
                    margin-bottom: 8px;
                }
                .principle-card p {
                    color: var(--text-secondary);
                    margin-bottom: 0;
                    font-size: 15px;
                }
                code {
                    font-family: var(--font-mono);
                    font-size: 13px;
                    background: var(--bg-muted);
                    padding: 2px 6px;
                    border-radius: 4px;
                }
            """),
        ),
        Body(
            A("← Back to Home", href="/", cls="nav-back"),
            H1("Design System"),
            P("Typography, spacing, and components for loopflow and loopflowstudio.", cls="subtitle"),

            Div(
                A("View Colors", href="/colors"),
                A("View Fonts", href="/fonts"),
                cls="nav-links",
            ),

            H2("Typography"),

            H3("Serif — Cormorant Garamond"),
            Div(
                Div("SERIF", cls="type-label"),
                Div("Agents that remember.", cls="type-serif"),
                Div("Work that compounds.", cls="type-serif-italic"),
                cls="type-sample",
            ),
            P("Used for headlines, taglines, and hero text. The italic "f" has an elongated S-curve reminiscent of a violin f-hole.", style="color: var(--text-secondary);"),

            H3("Sans — Lato"),
            Div(
                Div("SANS-SERIF", cls="type-label"),
                Div("Context that travels with your agents, prompts worth keeping, and tight feedback loops. Loopflow helps you build software with AI that actually works—no more starting from scratch every session.", cls="type-sans"),
                cls="type-sample",
            ),
            P("Used for body text, buttons, navigation, and UI elements. Warm humanist that pairs naturally with Cormorant Garamond.", style="color: var(--text-secondary);"),

            H3("Mono — JetBrains Mono"),
            Div(
                Pre(
                    Span("$ ", cls="prompt"), "lf init\n",
                    Span("$ ", cls="prompt"), "lf design\n",
                    Span("# Designing on branch: feature-auth", cls="comment"), "\n",
                    Span("$ ", cls="prompt"), "lf implement",
                    cls="type-mono",
                ),
                cls="type-sample",
            ),
            P("Used for code, terminal output, and technical content. Clear character distinction.", style="color: var(--text-secondary);"),

            H2("Spacing Scale"),
            P("Based on a 4pt grid. Use semantic names, not arbitrary values."),
            Div(
                Div(Div(style="width: 4px; height: 4px;", cls="spacing-box"), Div("xs", cls="spacing-label"), Div("4px", cls="spacing-value"), cls="spacing-item"),
                Div(Div(style="width: 8px; height: 8px;", cls="spacing-box"), Div("sm", cls="spacing-label"), Div("8px", cls="spacing-value"), cls="spacing-item"),
                Div(Div(style="width: 12px; height: 12px;", cls="spacing-box"), Div("md", cls="spacing-label"), Div("12px", cls="spacing-value"), cls="spacing-item"),
                Div(Div(style="width: 16px; height: 16px;", cls="spacing-box"), Div("lg", cls="spacing-label"), Div("16px", cls="spacing-value"), cls="spacing-item"),
                Div(Div(style="width: 20px; height: 20px;", cls="spacing-box"), Div("xl", cls="spacing-label"), Div("20px", cls="spacing-value"), cls="spacing-item"),
                Div(Div(style="width: 24px; height: 24px;", cls="spacing-box"), Div("xxl", cls="spacing-label"), Div("24px", cls="spacing-value"), cls="spacing-item"),
                Div(Div(style="width: 32px; height: 32px;", cls="spacing-box"), Div("xxxl", cls="spacing-label"), Div("32px", cls="spacing-value"), cls="spacing-item"),
                cls="spacing-grid",
            ),

            H2("Corner Radius"),
            Div(
                Div(Div(style="border-radius: 4px;", cls="radius-box"), Div("sm · 4px", cls="radius-label"), cls="radius-item"),
                Div(Div(style="border-radius: 8px;", cls="radius-box"), Div("md · 8px", cls="radius-label"), cls="radius-item"),
                Div(Div(style="border-radius: 12px;", cls="radius-box"), Div("lg · 12px", cls="radius-label"), cls="radius-item"),
                Div(Div(style="border-radius: 16px;", cls="radius-box"), Div("xl · 16px", cls="radius-label"), cls="radius-item"),
                Div(Div(style="border-radius: 9999px; width: 80px;", cls="radius-box"), Div("full", cls="radius-label"), cls="radius-item"),
                cls="radius-grid",
            ),

            H2("Buttons"),
            Div(
                Button("Primary Action", cls="btn btn-primary"),
                Button("Secondary Action", cls="btn btn-secondary"),
                cls="button-grid",
            ),
            P("Primary buttons use burgundy. One primary action per view.", style="color: var(--text-secondary); margin-top: 16px;"),

            H2("Design Principles"),

            Div(
                H4("Burgundy accent — used sparingly"),
                P("The accent color signals warmth, craft, and classical instruments. Use only for CTAs, links, and focus states. Never for backgrounds or large areas."),
                cls="principle-card",
            ),
            Div(
                H4("Cream backgrounds — clarity and warmth"),
                P("The warm cream palette (#FAF8F5) feels inviting without being cold. White surfaces (#FFFDFB) elevate cards and modals."),
                cls="principle-card",
            ),
            Div(
                H4("Serif for editorial moments"),
                P("Cormorant Garamond headlines signal intentionality and craft. Reserve for hero text, taglines, and special emphasis."),
                cls="principle-card",
            ),
            Div(
                H4("980px max-width — refined, not sprawling"),
                P("Narrower than typical SaaS (1200px). Signals that content is considered, not bloated."),
                cls="principle-card",
            ),

            H2("CSS Variables"),
            Pre(
                """:root {
  /* Brand */
  --burgundy: #722F37;
  --burgundy-hover: #8B3D47;

  /* Light mode */
  --bg: #FAF8F5;
  --bg-surface: #FFFDFB;
  --bg-muted: #F3EEE7;
  --border: #E3DDD5;
  --text: #1A1A1A;
  --text-secondary: #6B6B6B;

  /* Typography */
  --font-serif: 'Cormorant Garamond', Georgia, serif;
  --font-sans: 'Lato', -apple-system, sans-serif;
  --font-mono: 'JetBrains Mono', monospace;
}""",
                cls="type-mono",
            ),
        ),
    )


