"""Write the generated built-in themes, the prototype's theme table and the website's colors.

Each family below is a light and a dark theme; Loopflow Light and Loopflow Dark are
written by hand and only read here. loopflow.studio is drawn in Loopflow Light. Change a theme by editing its spec and rerunning; never
edit a generated file.

    uv run python scripts/theme_catalogue.py           # rewrite Catalogue/, desktop-themes.js and theme.css
    uv run python scripts/theme_catalogue.py --check   # fail if a committed file differs from a fresh run
"""
from __future__ import annotations

import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CATALOGUE = ROOT / "swift/Loopflow/Themes/Catalogue"
PROTOTYPE = ROOT / "design/prototypes/desktop-themes.js"
WEBSITE = ROOT / "website/static/theme.css"
HANDWRITTEN = ("Loopflow Light", "Loopflow Dark")

TERMINAL_KEYS = ["background", "foreground", "cursor-color", "cursor-text", "selection-background", "selection-foreground"]
ROLES = [
    "background", "surface", "surface-muted", "surface-element", "border", "border-strong",
    "border-active", "shadow", "text", "text-secondary", "text-tertiary", "emphasis-ink",
    "accent", "accent-hover", "accent-pressed", "accent-text", "accent-ink", "titlebar", "work-selected-fill",
    "work-selected-text", "work-selected-secondary-text", "selection-fill",
    "working-ink", "working-fill", "warning-ink", "warning-fill", "available-ink", "available-fill",
    "failure-ink", "failure-fill", "success-ink", "success-fill", "stopped-ink", "stopped-fill",
]
# The prototype's names for the roles it paints.
PROTOTYPE_ROLES = {
    "background": "background", "surface": "surface", "surface-muted": "surface-muted",
    "surface-element": "surface-element", "border": "border", "border-strong": "border-strong",
    "text": "text", "text-secondary": "text-secondary", "text-tertiary": "text-tertiary",
    "emphasis-ink": "emphasis-ink", "accent": "accent", "accent-text": "accent-text", "titlebar": "titlebar",
    "keyboard-focus": "accent-ink", "work-selected-fill": "work-selected-fill",
    "work-selected-text": "work-selected-text", "work-selected-secondary": "work-selected-secondary-text",
    "working": "working-ink", "waiting": "warning-ink", "available": "available-ink",
    "failure": "failure-ink", "success": "success-ink",
}
# The roles the website paints, under their own names.
WEBSITE_ROLES = [
    "background", "surface", "surface-muted", "surface-element", "border", "border-strong", "border-active",
    "shadow", "text", "text-secondary", "text-tertiary", "accent", "accent-hover", "accent-pressed",
    "accent-text", "accent-ink", "working-ink", "working-fill", "warning-ink", "warning-fill",
    "failure-ink", "failure-fill", "success-ink", "success-fill",
]
# `PaletteDefinition.titlebarSecondaryText`: the title bar's second text level.
TITLEBAR_SECONDARY = 0.72


# ── color, as `ThemeRGB` computes it ─────────────────────────────────────────

def rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) / 255 for i in (0, 2, 4))


def hexs(c):
    return "#%02x%02x%02x" % tuple(min(255, max(0, int(v * 255 + 0.5))) for v in c)


def lum(h):
    c = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in rgb(h)]
    return c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722


def cr(a, b):
    x, y = lum(a), lum(b)
    return (max(x, y) + 0.05) / (min(x, y) + 0.05)


def mix(a, b, amount):
    """`ThemeRGB.mixed(with:amount:)`: amount of b over a."""
    return hexs(tuple(x * (1 - amount) + y * amount for x, y in zip(rgb(a), rgb(b))))


def over(fg, alpha, bg):
    return mix(bg, fg, alpha)


def oklab(h):
    r, g, b = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in rgb(h)]
    l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b
    m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b
    s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b
    l, m, s = l ** (1 / 3), m ** (1 / 3), s ** (1 / 3)
    return (0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
            1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
            0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s)


def de(a, b):
    """OKLab distance ×100. About 2 is the smallest step most people see side by side."""
    return 100 * math.dist(oklab(a), oklab(b))


def ok(L, C, h):
    """OKLCH to sRGB hex, clipped to the gamut."""
    a, b = C * math.cos(math.radians(h)), C * math.sin(math.radians(h))
    l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3
    m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3
    s = (L - 0.0894841775 * a - 1.2914855480 * b) ** 3
    lin = (4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
           -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
           -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s)
    out = []
    for v in lin:
        v = min(1, max(0, v))
        out.append(12.92 * v if v <= 0.0031308 else 1.055 * v ** (1 / 2.4) - 0.055)
    return "#%02x%02x%02x" % tuple(int(v * 255 + 0.5) for v in out)


def prefers_light_text(h):
    return cr(h, "#ffffff") > cr(h, "#000000")


def readable(color, surfaces, dark, ratio=4.5):
    """`ThemeRGB.readable(on:dark:ratio:)`: toward white (dark) or black until every surface reads."""
    end = "#ffffff" if dark else "#000000"
    for step in range(256):
        v = mix(color, end, step / 255)
        if all(cr(v, s) >= ratio + 0.02 for s in surfaces):
            return v
    return end


def titlebar(bar):
    """`GhosttyThemeMapping.titlebar` past its dark mix: away from the bar's text until the second text level reads."""
    text = "#ffffff" if prefers_light_text(bar) else "#000000"
    end = "#000000" if text == "#ffffff" else "#ffffff"
    for step in range(256):
        v = mix(bar, end, step / 255)
        if cr(over(text, TITLEBAR_SECONDARY, v), v) >= 4.52:
            return v
    return end


# ── the families ─────────────────────────────────────────────────────────────

HUES = {"red": 27, "green": 148, "yellow": 88, "blue": 255, "magenta": 330, "cyan": 200}
STATUS_HUE = {"working": "blue", "warning": "yellow", "available": "magenta", "failure": "red", "success": "green"}

# One spec per appearance; `idea` heads both files.
# ground/text/accent are OKLCH. `on_accent` is the button text: "light" or "dark".
FAMILIES = [
    {
        "family": "Sorbet",
        "idea": "Pastels at one lightness on a violet-navy ground: every accent equally loud, none shouting.",
        "dark": dict(name="Sorbet", ground=(0.245, 0.035, 285), text=(0.92, 0.025, 285), accent=(0.80, 0.10, 350),
                     on_accent="dark", ink=(0.80, 0.10, 350), status_c=0.10, ansi_c=0.11,
                     hues=dict(blue=265, magenta=345, cyan=195), titlebar=(0.33, 0.06, 300)),
        "light": dict(name="Sorbet Day", ground=(0.972, 0.012, 300), text=(0.30, 0.05, 285), accent=(0.52, 0.15, 350),
                      on_accent="light", ink=(0.50, 0.15, 350), status_c=0.13, ansi_c=0.14,
                      hues=dict(blue=265, magenta=345, cyan=195), titlebar=(0.40, 0.10, 300)),
    },
    {
        "family": "Signal",
        "idea": "Saturated syntax on a neutral slate, the incumbent look: high chroma, clear separation.",
        "dark": dict(name="Signal", ground=(0.265, 0.018, 262), text=(0.90, 0.012, 262), accent=(0.60, 0.19, 300),
                     on_accent="light", ink=(0.78, 0.14, 300), status_c=0.15, ansi_c=0.17,
                     hues=dict(yellow=95, green=140), titlebar=(0.34, 0.10, 300)),
        "light": dict(name="Signal Light", ground=(0.985, 0.004, 262), text=(0.27, 0.02, 262), accent=(0.50, 0.21, 300),
                      on_accent="light", ink=(0.48, 0.20, 300), status_c=0.17, ansi_c=0.18,
                      hues=dict(yellow=80, green=145), titlebar="accent"),
    },
    {
        "family": "Tannin",
        "idea": "Roast and parchment with muted earth colors: warm, low glare, made for long sessions.",
        "dark": dict(name="Tannin", ground=(0.255, 0.016, 70), text=(0.89, 0.035, 85), accent=(0.70, 0.13, 58),
                     on_accent="dark", ink=(0.78, 0.11, 62), status_c=0.09, ansi_c=0.10,
                     hues=dict(green=125, blue=230, cyan=180, magenta=355, yellow=85), titlebar=(0.34, 0.05, 58)),
        "light": dict(name="Tannin Light", ground=(0.952, 0.03, 85), text=(0.30, 0.03, 60), accent=(0.52, 0.13, 50),
                      on_accent="light", ink=(0.48, 0.12, 50), status_c=0.10, ansi_c=0.11,
                      hues=dict(green=125, blue=230, cyan=180, magenta=355, yellow=80), titlebar="accent"),
    },
    {
        "family": "Fjord",
        "idea": "Calm blue-grey with frost accents: low chroma everywhere, a quiet room.",
        "dark": dict(name="Fjord", ground=(0.285, 0.027, 255), text=(0.93, 0.014, 240), accent=(0.76, 0.08, 215),
                     on_accent="dark", ink=(0.80, 0.08, 215), status_c=0.08, ansi_c=0.085,
                     hues=dict(blue=240, cyan=205), titlebar=(0.38, 0.04, 245)),
        "light": dict(name="Fjord Light", ground=(0.962, 0.01, 240), text=(0.31, 0.035, 255), accent=(0.48, 0.08, 212),
                      on_accent="light", ink=(0.47, 0.08, 212), status_c=0.09, ansi_c=0.10,
                      hues=dict(blue=240, cyan=205), titlebar="accent"),
    },
    {
        "family": "Folio",
        "idea": "Ink on newsprint, and the same inks reversed: one set of accents for both appearances.",
        "light": dict(name="Folio", ground=(0.975, 0.013, 95), text=(0.21, 0.01, 95), accent=(0.52, 0.17, 33),
                      on_accent="light", ink=(0.50, 0.16, 33), emphasis="text", status_c=0.14, ansi_c=0.15,
                      hues={}, titlebar=(0.24, 0.01, 95)),
        "dark": dict(name="Folio Night", ground=(0.19, 0.006, 95), text=(0.92, 0.014, 95), accent=(0.55, 0.17, 33),
                     on_accent="light", ink=(0.74, 0.14, 33), emphasis="text", status_c=0.13, ansi_c=0.14,
                     hues={}, titlebar=(0.27, 0.008, 95)),
    },
    {
        "family": "Arcade",
        "idea": "Neon on midnight, and a risograph print for day: the loud one, kept readable.",
        "dark": dict(name="Arcade", ground=(0.19, 0.06, 295), text=(0.93, 0.03, 300), accent=(0.72, 0.24, 345),
                     on_accent="dark", ink=(0.86, 0.12, 200), status_c=0.17, ansi_c=0.20,
                     hues=dict(magenta=345, cyan=200, blue=275, yellow=100), titlebar=(0.27, 0.12, 320)),
        "light": dict(name="Arcade Day", ground=(0.975, 0.006, 300), text=(0.26, 0.09, 290), accent=(0.55, 0.24, 350),
                      on_accent="light", ink=(0.46, 0.17, 250), status_c=0.17, ansi_c=0.19,
                      hues=dict(magenta=350, cyan=210, blue=265), titlebar=(0.40, 0.16, 290)),
    },
    {
        "family": "Graphite",
        "idea": "Almost no color. Structure by weight, one blue for action, and only an error is loud.",
        "dark": dict(name="Graphite", ground=(0.215, 0.0, 0), text=(0.91, 0.0, 0), accent=(0.58, 0.17, 258),
                     on_accent="light", ink=(0.78, 0.11, 258), emphasis="text", status_c=0.06, ansi_c=0.05,
                     loud="failure", hues={}, titlebar=(0.27, 0.0, 0)),
        "light": dict(name="Graphite Light", ground=(0.985, 0.0, 0), text=(0.22, 0.0, 0), accent=(0.50, 0.18, 258),
                      on_accent="light", ink=(0.48, 0.16, 258), emphasis="text", status_c=0.07, ansi_c=0.06,
                      loud="failure", hues={}, titlebar=(0.30, 0.0, 0)),
    },
    {
        "family": "Contrast",
        "idea": "7:1 for every word, pure ground, the selection drawn as an outline-strength fill.",
        "dark": dict(name="Contrast", ground=(0.0, 0.0, 0), text=(1.0, 0.0, 0), accent=(0.80, 0.16, 95),
                     on_accent="dark", ink=(0.90, 0.16, 95), status_c=0.13, ansi_c=0.15, ratio=7, secondary_L=0.92,
                     hues={}, titlebar=(0.0, 0.0, 0)),
        "light": dict(name="Contrast Light", ground=(1.0, 0.0, 0), text=(0.0, 0.0, 0), accent=(0.40, 0.17, 258),
                      on_accent="light", ink=(0.38, 0.17, 258), status_c=0.15, ansi_c=0.16, ratio=7, secondary_L=0.26,
                      hues={}, titlebar=(0.0, 0.0, 0)),
    },
    {
        "family": "Moss",
        "idea": "A forest floor: a deep moss ground with sulphur-yellow lichen as the accent by night, moss-green ink on pale lichen by day.",
        "dark": dict(name="Moss", ground=(0.265, 0.035, 128), text=(0.91, 0.03, 115), accent=(0.86, 0.13, 105),
                     on_accent="dark", ink=(0.86, 0.12, 105), status_c=0.10, ansi_c=0.11,
                     hues=dict(green=135, yellow=95, cyan=185), titlebar=(0.33, 0.05, 128)),
        "light": dict(name="Moss Light", ground=(0.95, 0.03, 112), text=(0.29, 0.04, 128), accent=(0.45, 0.10, 128),
                      on_accent="light", ink=(0.43, 0.10, 128), status_c=0.12, ansi_c=0.13,
                      hues=dict(green=135, yellow=90, cyan=185), titlebar=(0.36, 0.06, 128)),
    },
    {
        "family": "Fig",
        "idea": "The fruit and the leaf: a plum ground with a lime accent at night, a leaf-green ground with plum ink by day.",
        "dark": dict(name="Fig", ground=(0.24, 0.05, 345), text=(0.93, 0.02, 345), accent=(0.82, 0.16, 125),
                     on_accent="dark", ink=(0.84, 0.14, 125), status_c=0.11, ansi_c=0.12,
                     hues=dict(magenta=355, green=135), titlebar=(0.31, 0.08, 345)),
        "light": dict(name="Fig Leaf", ground=(0.965, 0.025, 125), text=(0.28, 0.07, 345), accent=(0.45, 0.14, 345),
                      on_accent="light", ink=(0.44, 0.14, 345), status_c=0.12, ansi_c=0.13,
                      hues=dict(magenta=355, green=135), titlebar=(0.38, 0.10, 345)),
    },
    {
        "family": "Terracotta",
        "idea": "Fired clay with a cobalt glaze: a burnt-orange ground and the deep blue painted over it.",
        "dark": dict(name="Terracotta", ground=(0.27, 0.045, 38), text=(0.92, 0.02, 60), accent=(0.72, 0.12, 268),
                     on_accent="dark", ink=(0.80, 0.10, 268), status_c=0.10, ansi_c=0.11,
                     hues=dict(red=18, blue=235), titlebar=(0.35, 0.07, 38)),
        "light": dict(name="Terracotta Light", ground=(0.95, 0.03, 50), text=(0.30, 0.05, 40), accent=(0.45, 0.15, 265),
                      on_accent="light", ink=(0.43, 0.15, 265), status_c=0.12, ansi_c=0.13,
                      hues=dict(red=18, blue=235), titlebar=(0.44, 0.11, 40)),
    },
]


def generate(spec, dark):
    ratio = spec.get("ratio", 4.5)
    L0, C0, h0 = spec["ground"]
    bg = ok(L0, C0, h0)
    step = (lambda d: ok(min(L0 + d, 0.995), C0, h0))
    if dark:
        ladder = [bg, step(0.028), step(0.05), step(0.075)]
    else:
        ladder = [bg, ok(min(L0 + 0.012, 1.0), C0 * 0.6, h0), step(-0.028), step(-0.05)]
    if spec.get("ratio") == 7:
        ladder = [bg, step(0.18 if dark else -0.015), step(0.24 if dark else -0.03), step(0.30 if dark else -0.045)]
    text = readable(ok(*spec["text"]), ladder, dark, ratio)

    # File selection: the text at the least opacity that shows on every surface.
    opacity = 0.04
    for o in range(4, 30):
        opacity = o / 100
        if min(de(over(text, opacity, s), s) for s in ladder) >= 3:
            break
    grounds = ladder + [over(text, opacity, s) for s in ladder]

    tL, tC, th = spec["text"]
    secondary = readable(ok(spec.get("secondary_L", 0.82 if dark else 0.40), tC * 0.8, th), grounds, dark, ratio)
    # Tertiary: as far toward the ground as it can go and still read.
    tertiary = secondary
    for amount in [x / 100 for x in range(45, 4, -1)]:
        candidate = mix(secondary, bg, amount)
        if all(cr(candidate, g) >= ratio + 0.02 for g in grounds):
            tertiary = candidate
            break
    ink = readable(ok(*spec["ink"]), grounds, dark, ratio)
    emphasis = text if spec.get("emphasis") == "text" else ink

    # The button: its own fill, with text that holds on rest, hover and pressed.
    fill = ok(*spec["accent"])
    if spec["on_accent"] == "dark":
        on = bg if L0 < 0.5 else text
        fill = readable(fill, [on], True, 4.6)
        hover, pressed = mix(fill, "#ffffff", 0.14), mix(fill, "#ffffff", 0.28)
    else:
        on = "#fbfaf8"
        fill = readable(fill, [on], False, 4.6)
        hover, pressed = mix(fill, "#000000", 0.14), mix(fill, "#000000", 0.28)

    # Work selection: far enough from the ground to show, near enough that focus still reads on it.
    selected = None
    for amount in [x / 100 for x in range(36 if ratio > 5 else 18, 6, -1)]:
        candidate = ok(L0 + (amount if dark else -amount), max(C0, 0.012) + 0.008, h0)
        if cr(ink, candidate) >= 3.05 and cr(candidate, ladder[1]) >= 1.25:
            selected = candidate
            break
    selected = selected or ok(L0 + (0.12 if dark else -0.08), C0, h0)
    sel_dark = prefers_light_text(selected)
    sel_text = readable(text, [selected], sel_dark, ratio)
    sel_secondary = readable(mix(sel_text, selected, 0.2), [selected], sel_dark, ratio)

    hues = dict(HUES, **spec["hues"])
    roles = {
        "background": bg, "surface": ladder[1], "surface-muted": ladder[2], "surface-element": ladder[3],
        "border": mix(bg, secondary, 0.2), "border-strong": mix(bg, secondary, 0.36),
        "border-active": readable(mix(bg, secondary, 0.55), ladder, dark, 3.05), "shadow": "#000000",
        "text": text, "text-secondary": secondary, "text-tertiary": tertiary, "emphasis-ink": emphasis,
        "accent": fill, "accent-hover": hover, "accent-pressed": pressed, "accent-text": on, "accent-ink": ink,
        "work-selected-fill": selected, "work-selected-text": sel_text, "work-selected-secondary-text": sel_secondary,
        "selection-fill": text,
    }
    for status, hue_name in STATUS_HUE.items():
        c = spec["status_c"] * (2.4 if spec.get("loud") == status else 1)
        tone = readable(ok(0.80 if dark else 0.50, c, hues[hue_name]), grounds, dark, ratio)
        # The tint is a saturated version of the hue, thinned: it shows as a color, not a grey.
        seed = ok(0.50 if dark else 0.72, max(c * 1.6, 0.12), hues[hue_name])
        for step_ in range(45, 0, -1):
            tint = mix(bg, seed, step_ / 100)
            if cr(tone, tint) >= ratio + 0.02 and cr(text, over(tone, 0.26, tint)) >= ratio + 0.02:
                break
        roles[status + "-ink"], roles[status + "-fill"] = tone, tint
    roles["stopped-ink"] = secondary
    roles["stopped-fill"] = mix(bg, secondary, 0.1)

    roles["titlebar"] = titlebar(fill if spec["titlebar"] == "accent" else ok(*spec["titlebar"]))

    # Terminal: the same ground and text as the window; the accent ink is the cursor.
    def ansi(name, L, C, r=4.5):
        return readable(ok(L, C, hues[name]), [bg], dark, r)
    normal_L, bright_L = (0.74, 0.83) if dark else (0.50, 0.44)
    bright_C = 1.1 if dark else 1.3
    a = [None] * 16
    for i, name in enumerate(["red", "green", "yellow", "blue", "magenta", "cyan"], start=1):
        c = spec["ansi_c"] * (2.4 if spec.get("loud") == "failure" and name == "red" else 1)
        a[i] = ansi(name, normal_L, c)
        a[i + 8] = ansi(name, bright_L, c * bright_C)
    if dark:
        a[0], a[8], a[7], a[15] = ok(L0 + 0.09, C0, h0), readable(ok(0.62, C0 * 1.4, h0), [bg], True, 4.5), secondary, text
    else:
        a[0], a[8], a[7], a[15] = text, readable(ok(0.56, C0 * 2, h0), [bg], False, 4.5), ok(0.84, C0, h0), ok(0.995, C0 * 0.5, h0)
    return {
        "name": spec["name"], "dark": dark, "ansi": a, "selection-opacity": opacity, "roles": roles,
        "term": {"background": bg, "foreground": text, "cursor-color": ink, "cursor-text": bg,
                 "selection-background": text, "selection-foreground": bg},
    }


def catalogue():
    """The generated themes, each family's light theme before its dark one."""
    themes = []
    for family in FAMILIES:
        for mode in ("light", "dark"):
            theme = generate(family[mode], mode == "dark")
            theme["idea"] = family["idea"]
            themes.append(theme)
    return themes


# ── files ────────────────────────────────────────────────────────────────────

def theme_file(t):
    lines = [f"# {t['name']}: Loopflow built-in theme, written by scripts/theme_catalogue.py.", f"# {t['idea']}"]
    lines += [f"{k} = {t['term'][k]}" for k in TERMINAL_KEYS]
    lines += [f"palette = {i}={c}" for i, c in enumerate(t["ansi"])]
    lines.append(f"loopflow-appearance = {'dark' if t['dark'] else 'light'}")
    lines += [f"loopflow-{k} = {t['roles'][k]}" for k in ROLES]
    lines.append(f"loopflow-selection-opacity = {t['selection-opacity']:g}")
    return "\n".join(lines) + "\n"


def read_theme(path):
    """A handwritten theme file in the shape the specs build: name, appearance, terminal, ANSI and roles."""
    t = {"name": path.name, "dark": False, "term": {}, "roles": {}}
    ansi = {}
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if line.startswith("#") or "=" not in line:
            continue
        key, value = [p.strip() for p in line.split("=", 1)]
        if key == "palette":
            index, color = value.split("=", 1)
            ansi[int(index)] = color.strip().lower()
        elif key in TERMINAL_KEYS:
            t["term"][key] = value.lower()
        elif key == "loopflow-appearance":
            t["dark"] = value == "dark"
        elif key.startswith("loopflow-"):
            t["roles"][key[len("loopflow-"):]] = value.lower()
    t["ansi"] = [ansi[i] for i in range(16)]
    return t


def prototype(themes):
    """`desktop-themes.js`: the roles the prototypes paint, in the app's order: Loopflow, then by name."""
    entries = [read_theme(CATALOGUE / name) for name in HANDWRITTEN] + sorted(themes, key=lambda t: t["name"])
    blocks = []
    for t in entries:
        rows = [f'"_mode": "{"dark" if t["dark"] else "light"}"']
        rows += [f'"{key}": "{t["roles"][role]}"' for key, role in PROTOTYPE_ROLES.items()]
        blocks.append(f'"{t["name"]}": {{\n' + ",\n".join(rows) + "\n}")
    header = "// Written by scripts/theme_catalogue.py from swift/Loopflow/Themes/Catalogue; do not edit.\n"
    return header + "var THEMES = {\n" + ",\n".join(blocks) + "\n};\n"


def website():
    """`theme.css`: Loopflow Light's roles and light terminal, the only colors the site has."""
    t = read_theme(CATALOGUE / "Loopflow Light")
    rows = [f"    --{role}: {t['roles'][role]};" for role in WEBSITE_ROLES]
    rows += [f"    --terminal-{key}: {t['term'][key]};" for key in ("background", "foreground")]
    rows += [f"    --terminal-{i}: {c};" for i, c in enumerate(t["ansi"])]
    header = "/* Written by scripts/theme_catalogue.py from Loopflow Light; do not edit. */\n"
    return header + ":root {\n" + "\n".join(rows) + "\n}\n"


def outputs():
    themes = catalogue()
    files = {CATALOGUE / t["name"]: theme_file(t) for t in themes}
    files[PROTOTYPE] = prototype(themes)
    files[WEBSITE] = website()
    return files


def stale(files):
    """Catalogue files neither handwritten nor generated."""
    keep = set(HANDWRITTEN) | {p.name for p in files if p.parent == CATALOGUE}
    return [p for p in CATALOGUE.iterdir() if p.is_file() and not p.name.startswith(".") and p.name not in keep]


def check():
    files = outputs()
    differ = [p for p, text in files.items() if not p.exists() or p.read_text() != text]
    return [str(p.relative_to(ROOT)) for p in differ + stale(files)]


def write():
    files = outputs()
    for path in stale(files):
        path.unlink()
    for path, text in files.items():
        path.write_text(text)


if __name__ == "__main__":
    if sys.argv[1:] == ["--check"]:
        differ = check()
        if differ:
            sys.exit("Run `uv run python scripts/theme_catalogue.py`; these differ from a fresh run:\n  " + "\n  ".join(differ))
    elif sys.argv[1:]:
        sys.exit("usage: theme_catalogue.py [--check]")
    else:
        write()
