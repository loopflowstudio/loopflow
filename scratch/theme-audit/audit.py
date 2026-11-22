"""Measure the bundled themes. `build.py` turns the result into the page."""
import colorsys
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CATALOGUE = ROOT / "swift/Loopflow/Themes/Catalogue"

APP_ROLES = [
    "background", "surface", "surface-muted", "surface-element", "border", "border-strong",
    "border-active", "shadow", "text", "text-secondary", "text-tertiary", "emphasis-ink",
    "accent", "accent-hover", "accent-pressed", "accent-text", "accent-ink", "work-selected-fill",
    "work-selected-text", "work-selected-secondary-text", "selection-fill",
    "working-ink", "working-fill", "warning-ink", "warning-fill", "available-ink", "available-fill",
    "failure-ink", "failure-fill", "success-ink", "success-fill", "stopped-ink", "stopped-fill",
]
STATUS = ["working", "warning", "available", "failure", "success"]
STATUS_WORD = {"working": "Busy", "warning": "Blocked", "available": "Available",
               "failure": "Error", "success": "Done", "stopped": "Stopped"}
ANSI_NAMES = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"]


# ── color ────────────────────────────────────────────────────────────────────

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


def lch(h):
    L, a, b = oklab(h)
    return L, math.hypot(a, b), math.degrees(math.atan2(b, a)) % 360


def hue_gap(a, b):
    d = abs(lch(a)[2] - lch(b)[2])
    return min(d, 360 - d)


def prefers_light_text(h):
    return cr(h, "#ffffff") > cr(h, "#000000")


def readable(color, surfaces, dark, ratio=4.5):
    end = "#ffffff" if dark else "#000000"
    for step in range(256):
        v = mix(color, end, step / 255)
        if all(cr(v, s) >= ratio + 0.02 for s in surfaces):
            return v
    return end


# ── theme files ──────────────────────────────────────────────────────────────

def parse(path):
    t = {"name": path.name, "ansi": [None] * 16, "term": {}, "roles": {}}
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        key, value = [p.strip() for p in line.split("=", 1)]
        if key == "palette":
            i, v = value.split("=")
            t["ansi"][int(i)] = v.strip().lower()
        elif key == "loopflow-appearance":
            t["dark"] = value == "dark"
        elif key == "loopflow-selection-opacity":
            t["selection-opacity"] = float(value)
        elif key.startswith("loopflow-"):
            t["roles"][key[len("loopflow-"):]] = value.lower()
        else:
            t["term"][key] = value.lower()
    return t


def load():
    order = {"Loopflow Light": 0, "Loopflow Dark": 1, "Linen": 2, "Hearth": 3}
    themes = [parse(p) for p in CATALOGUE.iterdir() if p.is_file() and not p.name.startswith(".")]
    return sorted(themes, key=lambda t: (order.get(t["name"], 4), t["dark"], t["name"]))


def mapping(t):
    """Port of `GhosttyThemeMapping.palette`: what an import with no app roles gets."""
    bg, fg, ansi = t["term"]["background"], t["term"]["foreground"], t["ansi"]
    dark = prefers_light_text(bg)
    end = "#ffffff" if dark else "#000000"

    def quiet(c, keep):
        r, g, b = rgb(c)
        gray = int((r * 0.2126 + g * 0.7152 + b * 0.0722) * 255)
        return mix("#%02x%02x%02x" % (gray, gray, gray), c, keep)

    def surface(amount):
        for step in range(int(amount * 1000), -1, -1):
            v = mix(bg, end, step / 1000)
            if cr(end, v) >= 4.55:
                return v
        return bg

    surfaces = [bg, surface(0.025), surface(0.045), surface(0.07)]
    foreground = quiet(fg, 0.15)
    text = readable(foreground, surfaces, dark)
    secondary = readable(mix(foreground, bg, 0.22), surfaces, dark)
    tertiary = readable(mix(foreground, bg, 0.38), surfaces, dark)
    seed = quiet(ansi[4], 0.55)
    accent_ink = readable(seed, surfaces, dark)
    accent = readable(seed, ["#faf8f5"], False)

    def tone(s):
        s = quiet(s, 0.6)
        fill = bg
        for step in range(7, -1, -1):
            c = mix(bg, s, step / 100)
            if cr(end, c) >= 4.55:
                fill = c
                break
        return readable(s, surfaces + [fill], dark), fill

    selected = readable(mix(bg, seed, 0.16), surfaces, dark, 3)
    sel_dark = prefers_light_text(selected)
    sel_text = readable(foreground, [selected], sel_dark)
    out = {
        "background": bg, "surface": surfaces[1], "surface-muted": surfaces[2], "surface-element": surfaces[3],
        "border": mix(bg, secondary, 0.22), "border-strong": mix(bg, secondary, 0.4), "border-active": accent_ink,
        "shadow": ansi[0], "text": text, "text-secondary": secondary, "text-tertiary": tertiary,
        "emphasis-ink": text, "accent": accent, "accent-hover": mix(accent, "#000000", 0.12),
        "accent-pressed": mix(accent, "#000000", 0.24), "accent-text": "#faf8f5", "accent-ink": accent_ink,
        "work-selected-fill": selected, "work-selected-text": sel_text,
        "work-selected-secondary-text": readable(mix(sel_text, selected, 0.16), [selected], sel_dark),
        "selection-fill": surfaces[1],
    }
    for name, s in [("working", ansi[4]), ("warning", ansi[3]), ("available", ansi[5]),
                    ("failure", ansi[1]), ("success", ansi[2]), ("stopped", secondary)]:
        out[name + "-ink"], out[name + "-fill"] = tone(s)
    return out


# ── derived surfaces the app paints ──────────────────────────────────────────

def derived(t):
    r = t["roles"]
    surfaces = {k: r[k] for k in ["background", "surface", "surface-muted", "surface-element"]}
    titlebar = mix(r["accent"], r["background"], 0.65) if t["dark"] else r["accent"]
    tb_text = "#ffffff" if prefers_light_text(r["accent"]) else "#000000"
    return {
        "surfaces": surfaces,
        "selected": {k: over(r["selection-fill"], t["selection-opacity"], v) for k, v in surfaces.items()},
        "titlebar": titlebar, "titlebar-text": tb_text,
        "titlebar-secondary": over(tb_text, 0.72, titlebar),
        "diff-selected": over(r["accent"], 0.16, r["background"]),
    }


# ── findings ─────────────────────────────────────────────────────────────────

def findings(t):
    """Each finding: (group, level, text). level: fail | close | note."""
    r, d, out = t["roles"], derived(t), []
    term = t["term"]

    def add(group, level, text):
        out.append((group, level, text))

    def text_pair(group, label, ink, base, floor=4.5, level="fail"):
        ratio = cr(ink, base)
        if ratio < floor:
            add(group, level, f"{label}: {ratio:.2f}:1, needs {floor:g}:1 ({ink} on {base})")

    inks = {"text": r["text"], "secondary text": r["text-secondary"], "tertiary text": r["text-tertiary"],
            "emphasis ink": r["emphasis-ink"], "accent ink": r["accent-ink"]}
    inks.update({STATUS_WORD[s] + " (" + s + ")": r[s + "-ink"] for s in STATUS})

    # Text on every surface and on the neutral selection over it.
    for label, ink in inks.items():
        for sname, base in d["surfaces"].items():
            text_pair("Text", f"{label} on {sname}", ink, base)
            text_pair("Text", f"{label} on selected {sname}", ink, d["selected"][sname])
    text_pair("Text", "selected row title", r["work-selected-text"], r["work-selected-fill"])
    text_pair("Text", "selected row second line", r["work-selected-secondary-text"], r["work-selected-fill"])
    for state in ["accent", "accent-hover", "accent-pressed"]:
        text_pair("Text", f"button text on {state}", r["accent-text"], r[state])
    text_pair("Text", "title bar path", d["titlebar-text"], d["titlebar"])
    text_pair("Text", "title bar secondary (72%)", d["titlebar-secondary"], d["titlebar"])
    for s in STATUS + ["stopped"]:
        text_pair("Text", f"{STATUS_WORD[s]} chip", r[s + "-ink"], r[s + "-fill"])
    # Marks.
    for sname, base in d["surfaces"].items():
        text_pair("Marks", f"keyboard focus ring on {sname}", r["accent-ink"], base, 3)
        text_pair("Marks", f"active pane border on {sname}", r["border-active"], base, 3)
    text_pair("Marks", "keyboard focus ring on the selected row", r["accent-ink"], r["work-selected-fill"], 3)
    side = cr(r["work-selected-fill"], r["surface"])
    if side < 1.2:
        add("Marks", "close", f"selected row barely differs from the sidebar: {side:.2f}:1 "
            f"({r['work-selected-fill']} on {r['surface']})")
    worst = min(de(d["selected"][k], v) for k, v in d["surfaces"].items())
    flat = [k for k, v in d["surfaces"].items() if de(d["selected"][k], v) < 1]
    if flat:
        add("Marks", "fail", f"file selection fill is invisible on {', '.join(flat)}: "
            f"{r['selection-fill']} at {t['selection-opacity']:g} changes the surface by ΔE {worst:.1f}; "
            "only the 2 pt edge says a file is selected")

    # Roles that collapse.
    if de(r["text-secondary"], r["text-tertiary"]) < 2:
        add("Roles", "fail", f"secondary and tertiary text are one color: {r['text-secondary']} and "
            f"{r['text-tertiary']} (ΔE {de(r['text-secondary'], r['text-tertiary']):.1f})")
    if de(r["text"], r["text-secondary"]) < 6:
        add("Roles", "close", f"text and secondary text are close: ΔE {de(r['text'], r['text-secondary']):.1f}")
    if r["emphasis-ink"] == r["text"]:
        add("Roles", "note", "emphasis ink is the text color: headings and repo names carry no color")
    if r["accent-hover"] == r["accent-pressed"]:
        add("Roles", "fail", f"accent button hover and pressed are the same fill ({r['accent-hover']})")
    elif de(r["accent"], r["accent-hover"]) < 2:
        add("Roles", "close", "accent button hover is indistinguishable from rest")
    names = STATUS + ["stopped"]
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            gap = de(r[a + "-ink"], r[b + "-ink"])
            if gap < 5:
                add("Roles", "fail" if gap < 3 else "close",
                    f"{STATUS_WORD[a]} and {STATUS_WORD[b]} are {'the same color' if gap < 3 else 'close'}: "
                    f"{r[a + '-ink']} and {r[b + '-ink']} (ΔE {gap:.1f})")
    for s in STATUS:
        gap = de(r[s + "-ink"], r["text-secondary"])
        if gap < 4:
            add("Roles", "close", f"{STATUS_WORD[s]} reads as secondary text: {r[s + '-ink']} vs "
                f"{r['text-secondary']} (ΔE {gap:.1f})")
        if lch(r[s + "-ink"])[1] < 0.03:
            add("Roles", "close", f"{STATUS_WORD[s]} has almost no color: {r[s + '-ink']} (chroma "
                f"{lch(r[s + '-ink'])[1]:.3f})")
    ladder = ["background", "surface", "surface-muted", "surface-element"]
    for a, b in zip(ladder, ladder[1:]):
        if de(r[a], r[b]) < 1:
            add("Roles", "close", f"{a} and {b} are one step: ΔE {de(r[a], r[b]):.1f}")
    if cr(r["border"], r["background"]) < 1.12 and cr(r["border"], r["surface"]) < 1.12:
        add("Roles", "close", f"border is barely there: {cr(r['border'], r['background']):.2f}:1 on background")

    # Fallback.
    m = mapping(t)
    same = [k for k in APP_ROLES if k in m and m[k] == r[k] and k != "background"]
    hand = [k for k in APP_ROLES if k in m and m[k] != r[k]]
    t["derived_count"], t["hand"] = len(same), hand
    if len(same) >= 20:
        add("Roles", "note", f"{len(same)} of 32 app roles are exactly what the import mapping derives from the "
            f"terminal colors; authored by hand: {', '.join(hand) or 'none'}")

    # Terminal.
    tbg = term["background"]
    low3, low45 = [], []
    for i, c in enumerate(t["ansi"]):
        ratio = cr(c, tbg)
        name = ("bright " if i > 7 else "") + ANSI_NAMES[i % 8]
        ground = (i in (0,) and t["dark"]) or (i in (7, 15) and not t["dark"]) or (i == 8 and t["dark"] and False)
        if ground:
            continue
        if ratio < 3:
            low3.append(f"{i} {name} {ratio:.1f}")
        elif ratio < 4.5:
            low45.append(f"{i} {name} {ratio:.1f}")
    if low3:
        add("Terminal", "fail", "ANSI under 3:1 on the terminal background, so Ghostty's minimum-contrast shifts "
            "them: " + ", ".join(low3))
    if low45:
        add("Terminal", "close", "ANSI between 3:1 and 4.5:1: " + ", ".join(low45))
    dup = [f"{i}={i + 8}" for i in range(8) if t["ansi"][i] == t["ansi"][i + 8]]
    if len(dup) >= 4:
        add("Terminal", "close", f"bright colors repeat the normal ones ({', '.join(dup)}): bold and bright "
            "output look alike")
    if tbg != r["background"]:
        add("Terminal", "note", f"terminal background {tbg} differs from the app background {r['background']}")
    if de(term["foreground"], r["text"]) > 4:
        add("Terminal", "note", f"terminal text {term['foreground']} and app text {r['text']} differ "
            f"(ΔE {de(term['foreground'], r['text']):.1f})")
    for s, i in [("failure", 1), ("success", 2), ("warning", 3), ("working", 4)]:
        ink, a = r[s + "-ink"], t["ansi"][i]
        if lch(ink)[1] > 0.03 and lch(a)[1] > 0.03 and hue_gap(ink, a) > 35:
            add("Terminal", "close", f"{STATUS_WORD[s]} in the interface ({ink}) is a different hue from terminal "
                f"{ANSI_NAMES[i]} ({a}): {hue_gap(ink, a):.0f}° apart")
    if lch(r["accent"])[1] > 0.04:
        nearest = min(hue_gap(r["accent"], c) for c in t["ansi"] if lch(c)[1] > 0.04) if any(
            lch(c)[1] > 0.04 for c in t["ansi"]) else 0
        if nearest > 40:
            add("Terminal", "note", f"the accent's hue appears nowhere in the terminal palette (nearest ANSI "
                f"is {nearest:.0f}° away)")

    # Diff.
    for s, word in [("success", "added"), ("failure", "removed")]:
        fill, ink = r[s + "-fill"], r[s + "-ink"]
        gap = de(fill, r["background"])
        if gap < 2:
            add("Diff", "fail", f"{word} line tint is invisible: {fill} on {r['background']} (ΔE {gap:.1f})")
        elif gap < 3.5:
            add("Diff", "close", f"{word} line tint is faint: {fill} on {r['background']} (ΔE {gap:.1f})")
        text_pair("Diff", f"code on the {word} tint", r["text"], fill)
        text_pair("Diff", f"code on a changed word in a {word} line", r["text"], over(ink, 0.26, fill))
        text_pair("Diff", f"{'+' if s == 'success' else '−'} marker (80%) on the {word} tint",
                  over(ink, 0.8, fill), fill, 3)
    gap = de(r["success-fill"], r["failure-fill"])
    if gap < 4:
        add("Diff", "fail" if gap < 2.5 else "close", f"added and removed tints are "
            f"{'the same' if gap < 2.5 else 'close'}: {r['success-fill']} and {r['failure-fill']} (ΔE {gap:.1f})")
    text_pair("Diff", "code on the selected diff line (accent 16%)", r["text"], d["diff-selected"])
    return out


def theme_distance(a, b):
    keys = ["background", "surface", "text", "text-secondary", "accent", "accent-ink", "work-selected-fill"] + [
        s + "-ink" for s in STATUS]
    app = sum(de(a["roles"][k], b["roles"][k]) for k in keys) / len(keys)
    term = sum(de(x, y) for x, y in zip(a["ansi"], b["ansi"])) / 16
    return app, term


if __name__ == "__main__":
    for t in load():
        fs = findings(t)
        print(f"\n== {t['name']} ({'dark' if t['dark'] else 'light'}) derived {t['derived_count']}/32")
        for g, level, text in fs:
            print(f"  [{g}/{level}] {text}")
