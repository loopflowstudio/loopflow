"""A proposed catalogue: themes generated from a short spec, in light and dark pairs.

Each family covers ground a group of well-loved themes holds, with its own values.
No upstream theme's colors are used. Every theme comes out of one generator, so a
rule fixed here is fixed in all of them.
"""
from audit import TITLEBAR_SECONDARY, cr, de, lch, mix, over, prefers_light_text, readable
from proposals import from_oklch as ok

HUES = {"red": 27, "green": 148, "yellow": 88, "blue": 255, "magenta": 330, "cyan": 200}
STATUS_HUE = {"working": "blue", "warning": "yellow", "available": "magenta", "failure": "red", "success": "green"}

# Each family: what it answers in the research, then one spec per appearance.
# ground/text/accent are OKLCH. `on_accent` is the button text: "light" or "dark".
FAMILIES = [
    {
        "family": "Sorbet", "answers": "Catppuccin, Rosé Pine, Tokyo Night",
        "idea": "Pastels at one lightness on a violet-navy ground: every accent equally loud, none shouting.",
        "dark": dict(name="Sorbet", ground=(0.245, 0.035, 285), text=(0.92, 0.025, 285), accent=(0.80, 0.10, 350),
                     on_accent="dark", ink=(0.80, 0.10, 350), status_c=0.10, ansi_c=0.11,
                     hues=dict(blue=265, magenta=345, cyan=195), titlebar=(0.33, 0.06, 300)),
        "light": dict(name="Sorbet Day", ground=(0.972, 0.012, 300), text=(0.30, 0.05, 285), accent=(0.52, 0.15, 350),
                      on_accent="light", ink=(0.50, 0.15, 350), status_c=0.13, ansi_c=0.14,
                      hues=dict(blue=265, magenta=345, cyan=195), titlebar=(0.40, 0.10, 300)),
    },
    {
        "family": "Signal", "answers": "One Dark, Dracula, Monokai",
        "idea": "Saturated syntax on a neutral slate, the incumbent look: high chroma, clear separation.",
        "dark": dict(name="Signal", ground=(0.265, 0.018, 262), text=(0.90, 0.012, 262), accent=(0.60, 0.19, 300),
                     on_accent="light", ink=(0.78, 0.14, 300), status_c=0.15, ansi_c=0.17,
                     hues=dict(yellow=95, green=140), titlebar=(0.34, 0.10, 300)),
        "light": dict(name="Signal Light", ground=(0.985, 0.004, 262), text=(0.27, 0.02, 262), accent=(0.50, 0.21, 300),
                      on_accent="light", ink=(0.48, 0.20, 300), status_c=0.17, ansi_c=0.18,
                      hues=dict(yellow=80, green=145), titlebar="accent"),
    },
    {
        "family": "Harbor", "answers": "Night Owl, Material, Cobalt2, Winter is Coming, Ayu",
        "idea": "Deep navy with an amber lamp: the blue-black editor ground, warmed by one color.",
        "dark": dict(name="Harbor", ground=(0.205, 0.05, 248), text=(0.90, 0.03, 230), accent=(0.78, 0.14, 75),
                     on_accent="dark", ink=(0.80, 0.13, 75), status_c=0.12, ansi_c=0.13,
                     hues=dict(cyan=210, blue=245), titlebar=(0.29, 0.07, 248)),
        "light": dict(name="Harbor Light", ground=(0.968, 0.014, 230), text=(0.27, 0.07, 250), accent=(0.45, 0.13, 250),
                      on_accent="light", ink=(0.48, 0.13, 250), status_c=0.13, ansi_c=0.14,
                      hues=dict(cyan=210, blue=245), titlebar="accent"),
    },
    {
        "family": "Tannin", "answers": "Gruvbox, Everforest, Kanagawa, Melange",
        "idea": "Roast and parchment with muted earth colors: warm, low glare, made for long sessions.",
        "dark": dict(name="Tannin", ground=(0.255, 0.016, 70), text=(0.89, 0.035, 85), accent=(0.70, 0.13, 58),
                     on_accent="dark", ink=(0.78, 0.11, 62), status_c=0.09, ansi_c=0.10,
                     hues=dict(green=125, blue=230, cyan=180, magenta=355, yellow=85), titlebar=(0.34, 0.05, 58)),
        "light": dict(name="Tannin Light", ground=(0.952, 0.03, 85), text=(0.30, 0.03, 60), accent=(0.52, 0.13, 50),
                      on_accent="light", ink=(0.48, 0.12, 50), status_c=0.10, ansi_c=0.11,
                      hues=dict(green=125, blue=230, cyan=180, magenta=355, yellow=80), titlebar="accent"),
    },
    {
        "family": "Fjord", "answers": "Nord",
        "idea": "Calm blue-grey with frost accents: low chroma everywhere, a quiet room.",
        "dark": dict(name="Fjord", ground=(0.285, 0.027, 255), text=(0.93, 0.014, 240), accent=(0.76, 0.08, 215),
                     on_accent="dark", ink=(0.80, 0.08, 215), status_c=0.08, ansi_c=0.085,
                     hues=dict(blue=240, cyan=205), titlebar=(0.38, 0.04, 245)),
        "light": dict(name="Fjord Light", ground=(0.962, 0.01, 240), text=(0.31, 0.035, 255), accent=(0.48, 0.08, 212),
                      on_accent="light", ink=(0.47, 0.08, 212), status_c=0.09, ansi_c=0.10,
                      hues=dict(blue=240, cyan=205), titlebar="accent"),
    },
    {
        "family": "Folio", "answers": "Solarized, Flexoki, GitHub",
        "idea": "Ink on newsprint, and the same inks reversed: one set of accents for both appearances.",
        "light": dict(name="Folio", ground=(0.975, 0.013, 95), text=(0.21, 0.01, 95), accent=(0.52, 0.17, 33),
                      on_accent="light", ink=(0.50, 0.16, 33), emphasis="text", status_c=0.14, ansi_c=0.15,
                      hues={}, titlebar=(0.24, 0.01, 95)),
        "dark": dict(name="Folio Night", ground=(0.19, 0.006, 95), text=(0.92, 0.014, 95), accent=(0.55, 0.17, 33),
                     on_accent="light", ink=(0.74, 0.14, 33), emphasis="text", status_c=0.13, ansi_c=0.14,
                     hues={}, titlebar=(0.27, 0.008, 95)),
    },
    {
        "family": "Arcade", "answers": "SynthWave '84, Shades of Purple, Andromeda, Cyberdream",
        "idea": "Neon on midnight, and a risograph print for day: the loud one, kept readable.",
        "dark": dict(name="Arcade", ground=(0.19, 0.06, 295), text=(0.93, 0.03, 300), accent=(0.72, 0.24, 345),
                     on_accent="dark", ink=(0.86, 0.12, 200), status_c=0.17, ansi_c=0.20,
                     hues=dict(magenta=345, cyan=200, blue=275, yellow=100), titlebar=(0.27, 0.12, 320)),
        "light": dict(name="Arcade Day", ground=(0.962, 0.02, 350), text=(0.26, 0.09, 290), accent=(0.55, 0.24, 350),
                      on_accent="light", ink=(0.46, 0.17, 250), status_c=0.17, ansi_c=0.19,
                      hues=dict(magenta=350, cyan=210, blue=265), titlebar=(0.40, 0.16, 290)),
    },
    {
        "family": "Graphite", "answers": "Min, zenbones, Alabaster, oxocarbon",
        "idea": "Almost no color. Structure by weight, one blue for action, and only an error is loud.",
        "dark": dict(name="Graphite", ground=(0.215, 0.0, 0), text=(0.91, 0.0, 0), accent=(0.58, 0.17, 258),
                     on_accent="light", ink=(0.78, 0.11, 258), emphasis="text", status_c=0.06, ansi_c=0.05,
                     loud="failure", hues={}, titlebar=(0.27, 0.0, 0)),
        "light": dict(name="Graphite Light", ground=(0.985, 0.0, 0), text=(0.22, 0.0, 0), accent=(0.50, 0.18, 258),
                      on_accent="light", ink=(0.48, 0.16, 258), emphasis="text", status_c=0.07, ansi_c=0.06,
                      loud="failure", hues={}, titlebar=(0.30, 0.0, 0)),
    },
    {
        "family": "Contrast", "answers": "high-contrast modes in macOS, VS Code and JetBrains",
        "idea": "7:1 for every word, pure ground, the selection drawn as an outline-strength fill.",
        "dark": dict(name="Contrast", ground=(0.0, 0.0, 0), text=(1.0, 0.0, 0), accent=(0.80, 0.16, 95),
                     on_accent="dark", ink=(0.90, 0.16, 95), status_c=0.13, ansi_c=0.15, ratio=7, secondary_L=0.92,
                     hues={}, titlebar=(0.0, 0.0, 0)),
        "light": dict(name="Contrast Light", ground=(1.0, 0.0, 0), text=(0.0, 0.0, 0), accent=(0.40, 0.17, 258),
                      on_accent="light", ink=(0.38, 0.17, 258), status_c=0.15, ansi_c=0.16, ratio=7, secondary_L=0.26,
                      hues={}, titlebar=(0.0, 0.0, 0)),
    },
    {
        "family": "Verdigris", "gap": "Nothing loved holds a green-cyan ground; the map is empty there.",
        "idea": "Oxidized copper: a patina-green ground with the bright copper it grew on as the accent.",
        "dark": dict(name="Verdigris", ground=(0.26, 0.04, 175), text=(0.92, 0.025, 165), accent=(0.70, 0.12, 28),
                     on_accent="dark", ink=(0.80, 0.09, 30), status_c=0.10, ansi_c=0.11,
                     hues=dict(green=140, cyan=195), titlebar=(0.34, 0.06, 175)),
        "light": dict(name="Verdigris Light", ground=(0.965, 0.022, 172), text=(0.28, 0.04, 175), accent=(0.52, 0.13, 28),
                      on_accent="light", ink=(0.48, 0.12, 28), status_c=0.12, ansi_c=0.13,
                      hues=dict(green=140, cyan=195), titlebar=(0.36, 0.07, 175)),
    },
    {
        "family": "Fig", "gap": "Magenta grounds are empty too, and no loved theme swaps its hues between light and dark.",
        "idea": "The fruit and the leaf: a plum ground with a lime accent at night, a leaf-green ground with plum ink by day.",
        "dark": dict(name="Fig", ground=(0.24, 0.05, 345), text=(0.93, 0.02, 345), accent=(0.82, 0.16, 125),
                     on_accent="dark", ink=(0.84, 0.14, 125), status_c=0.11, ansi_c=0.12,
                     hues=dict(magenta=355, green=135), titlebar=(0.31, 0.08, 345)),
        "light": dict(name="Fig Leaf", ground=(0.965, 0.025, 125), text=(0.28, 0.07, 345), accent=(0.45, 0.14, 345),
                      on_accent="light", ink=(0.44, 0.14, 345), status_c=0.12, ansi_c=0.13,
                      hues=dict(magenta=355, green=135), titlebar=(0.38, 0.10, 345)),
    },
    {
        "family": "Terracotta", "gap": "Red-orange grounds are empty: loved warm themes stop at brown.",
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

    # Title bar: stated, and dark enough (or light enough) for its secondary text.
    bar = fill if spec["titlebar"] == "accent" else ok(*spec["titlebar"])
    for _ in range(200):
        on_bar = "#ffffff" if prefers_light_text(bar) else "#000000"
        if cr(over(on_bar, TITLEBAR_SECONDARY, bar), bar) >= 4.55:
            break
        bar = mix(bar, "#000000" if on_bar == "#ffffff" else "#ffffff", 0.03)
    roles["titlebar"] = bar

    # Terminal: the same ground and text as the window; the accent ink is the cursor.
    tb = bg
    def ansi(name, L, C, r=4.5):
        return readable(ok(L, C, hues[name]), [tb], dark, r)
    normal_L, bright_L = (0.74, 0.83) if dark else (0.50, 0.44)
    bright_C = 1.1 if dark else 1.3
    a = [None] * 16
    for i, name in enumerate(["red", "green", "yellow", "blue", "magenta", "cyan"], start=1):
        c = spec["ansi_c"] * (2.4 if spec.get("loud") == "failure" and name == "red" else 1)
        a[i] = ansi(name, normal_L, c)
        a[i + 8] = ansi(name, bright_L, c * bright_C)
    if dark:
        a[0], a[8], a[7], a[15] = ok(L0 + 0.09, C0, h0), readable(ok(0.62, C0 * 1.4, h0), [tb], True, 4.5), secondary, text
    else:
        a[0], a[8], a[7], a[15] = text, readable(ok(0.56, C0 * 2, h0), [tb], False, 4.5), ok(0.84, C0, h0), ok(0.995, C0 * 0.5, h0)
    return {
        "name": spec["name"], "dark": dark, "ansi": a, "selection-opacity": opacity, "roles": roles,
        "term": {"background": tb, "foreground": text, "cursor-color": ink, "cursor-text": tb,
                 "selection-background": text, "selection-foreground": tb},
    }


def catalogue():
    out = []
    for fam in FAMILIES:
        for mode in ("light", "dark"):
            t = generate(fam[mode], mode == "dark")
            t["family"], t["idea"] = fam["family"], fam["idea"]
            t["answers"], t["gap"] = fam.get("answers"), fam.get("gap")
            out.append(t)
    return out


if __name__ == "__main__":
    from audit import findings
    for t in catalogue():
        fs = [f for f in findings(t) if f[1] != "note"]
        r = t["roles"]
        print(f"{t['name']:16} bg {r['background']} bar {r['titlebar']} accent {r['accent']} op {t['selection-opacity']:.2f}"
              f"  {len([f for f in fs if f[1] == 'fail'])} fail {len([f for f in fs if f[1] == 'close'])} close")
        for f in fs:
            print("     ", f)
