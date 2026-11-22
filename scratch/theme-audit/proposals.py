"""Proposed values for the two defaults. Nothing here touches the theme files."""
import copy
import math

from audit import STATUS, cr, de, derived, lch, mix, over, readable, titlebar_rule


def from_oklch(L, C, h):
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


def _finish(t):
    """Settle everything that depends on the ground: selection, inks, fills, focus."""
    r, dark = t["roles"], t["dark"]
    ladder = [r[k] for k in ["background", "surface", "surface-muted", "surface-element"]]
    # File selection: the text color at low opacity tints every surface the same amount.
    r["selection-fill"] = r["text"]
    for step in range(4, 30):
        t["selection-opacity"] = step / 100
        if min(de(over(r["text"], step / 100, s), s) for s in ladder) >= 3:
            break
    grounds = ladder + [over(r["selection-fill"], t["selection-opacity"], s) for s in ladder]
    for k in ["text-secondary", "emphasis-ink", "accent-ink"] + [s + "-ink" for s in STATUS]:
        r[k] = readable(r[k], grounds, dark)
    r["text-tertiary"] = readable(mix(r["text-secondary"], r["background"], 0.3), grounds, dark)
    r["stopped-ink"] = r["text-secondary"]
    r["border-active"] = readable(r["border-active"], ladder, dark, 3)
    # One fill serves the status chip and the diff line, so it has to be seen.
    for s in STATUS + ["stopped"]:
        ink = r[s + "-ink"]
        for step in range(18, 0, -1):
            fill = mix(r["background"], ink, step / 100)
            if cr(ink, fill) >= 4.52 and cr(r["text"], over(ink, 0.26, fill)) >= 4.52:
                break
        r[s + "-fill"] = fill
    return t


def light_repair(light):
    t = copy.deepcopy(light)
    r = t["roles"]
    r["accent-pressed"] = "#4c1f27"
    r["titlebar"] = titlebar_rule(r["accent"], r["background"], False)
    # Bright ANSI: the same hues, lighter and more saturated, still 4.5:1 on the ground.
    for i in range(1, 7):
        L, C, h = lch(t["ansi"][i])
        t["ansi"][i + 8] = readable(from_oklch(L + 0.07, C * 1.25, h), [t["term"]["background"]], False)
    return _finish(t)


def dark_repair(dark):
    """Same slate and bronze; only what fails changes."""
    t = copy.deepcopy(dark)
    r = t["roles"]
    bg = r["background"]
    # A shallower ladder leaves room for a third text level.
    for k, keep in [("surface", 0.55), ("surface-muted", 0.6), ("surface-element", 0.62)]:
        r[k] = mix(bg, r[k], keep)
    for s, hue in [("working", 254), ("warning", 83), ("available", 344), ("failure", 24), ("success", 153)]:
        r[s + "-ink"] = from_oklch(0.80, 0.09 if s != "available" else 0.07, hue)
    for i in list(range(1, 7)):
        t["ansi"][i] = readable(t["ansi"][i], [t["term"]["background"]], True)
    t["ansi"][8] = readable(t["ansi"][8], [t["term"]["background"]], True, 3.6)
    return _finish(t)


def dark_pair(dark, light, cool=0.0):
    """Loopflow Light turned over: its ink becomes the ground, burgundy becomes the ink.

    `cool` moves the ground from Light's warm ink (0) to today's slate (1).
    """
    t = dark_repair(dark)
    r, term = t["roles"], t["term"]
    bg = mix(light["roles"]["text"], dark["roles"]["background"], cool)
    paper = mix(light["roles"]["background"], "#e6edf5", cool)
    r["background"] = term["background"] = term["cursor-text"] = term["selection-foreground"] = bg
    r["surface"], r["surface-muted"], r["surface-element"] = mix(bg, paper, 0.035), mix(bg, paper, 0.065), mix(bg, paper, 0.10)
    r["work-selected-fill"] = mix(bg, paper, 0.24)
    r["text-secondary"] = mix(r["text"], bg, 0.22)
    r["border"], r["border-strong"] = mix(bg, r["text-secondary"], 0.2), mix(bg, r["text-secondary"], 0.36)
    r["border-active"] = mix(bg, r["text-secondary"], 0.5)
    r["accent-ink"] = r["emphasis-ink"] = from_oklch(0.78, 0.085, 15)
    r["accent"], r["accent-hover"], r["accent-pressed"] = "#8a3a44", "#722f37", "#5e2630"
    # The one bar the rule does not serve: Light's own, so the two defaults share it.
    r["titlebar"] = light["roles"]["accent"]
    r["available-ink"] = from_oklch(0.80, 0.075, 340)
    r["failure-ink"] = from_oklch(0.78, 0.11, 32)
    t["ansi"][5] = from_oklch(0.72, 0.10, 15)
    t["ansi"][13] = from_oklch(0.82, 0.085, 15)
    for i in list(range(1, 7)) + list(range(9, 15)):
        t["ansi"][i] = readable(t["ansi"][i], [bg], True)
    t["ansi"][8] = readable(t["ansi"][8], [bg], True, 3.6)
    _finish(t)
    for k in ["work-selected-text", "work-selected-secondary-text"]:
        r[k] = readable(r[k], [r["work-selected-fill"]], True)
    return t


def changes(before, after):
    rows = []
    for k, v in after["roles"].items():
        if k in before["roles"] and before["roles"][k] != v:
            rows.append((k, before["roles"][k], v))
    for k, v in after["term"].items():
        if before["term"][k] != v:
            rows.append(("terminal " + k, before["term"][k], v))
    for i, v in enumerate(after["ansi"]):
        if before["ansi"][i] != v:
            rows.append((f"ANSI {i}", before["ansi"][i], v))
    if before["selection-opacity"] != after["selection-opacity"]:
        rows.append(("selection-opacity", f"{before['selection-opacity']:g}", f"{after['selection-opacity']:g}"))
    return rows


def pair_facts(light, dark):
    out = []
    for t in (light, dark):
        r, d = t["roles"], derived(t)
        out.append({
            "name": t["name"], "ground": r["background"], "ground_hue": lch(r["background"])[2],
            "ground_chroma": lch(r["background"])[1], "accent": r["accent"], "accent_ink": r["accent-ink"],
            "accent_ink_hue": lch(r["accent-ink"])[2], "titlebar": d["titlebar"],
            "titlebar_ratio": cr(d["titlebar"], r["background"]), "button_ratio": cr(r["accent"], r["background"]),
            "emphasis": r["emphasis-ink"],
        })
    return out
