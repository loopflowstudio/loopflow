"""Write index.html: every bundled theme on the same three screens, with what fails beside it.

    python3 -I scratch/theme-audit/build.py
"""
import html
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from audit import ANSI_NAMES, STATUS, STATUS_WORD, cr, de, derived, findings, hue_gap, lch, load, theme_distance  # noqa: E402
from proposals import changes, dark_pair, dark_repair, light_repair, pair_facts  # noqa: E402

GROUPS = ["Text", "Marks", "Roles", "Terminal", "Diff"]
LEVEL_WORD = {"fail": "Fails", "close": "Close", "note": "Note"}


def e(s):
    return html.escape(str(s))


def slug(name):
    return name.lower().replace(" ", "-")


def css_vars(t):
    r, d = t["roles"], derived(t)
    pairs = {k: v for k, v in r.items()}
    pairs.update({f"a{i}": c for i, c in enumerate(t["ansi"])})
    pairs.update({"term-bg": t["term"]["background"], "term-fg": t["term"]["foreground"],
                  "term-sel-bg": t["term"]["selection-background"], "term-sel-fg": t["term"]["selection-foreground"],
                  "cursor": t["term"]["cursor-color"], "cursor-text": t["term"]["cursor-text"],
                  "titlebar": d["titlebar"], "titlebar-text": d["titlebar-text"],
                  "titlebar-2": d["titlebar-secondary"], "sel-op": f"{t['selection-opacity'] * 100:g}%"})
    return ";".join(f"--{k}:{v}" for k, v in pairs.items())


def status(s, word=None):
    return f'<span class="st {s}"><i></i>{word or STATUS_WORD[s]}</span>'


def workspace(t):
    rows = [
        ("Review the bundled themes against the design system", "50f7f9", "warning", "2m", True,
         "Which Loopflow Dark do you want built?"),
        ("Mark where the changes are on the Diff’s scrollbar", "07d8c9", "working", "14s", False, None),
        ("Keep the enclosing function in view while a diff scrolls", "268d5a", "available", "1h", False, None),
        ("Read Loopflow docs offline with lf docs", "fd90d7", "failure", "3m", False, None),
        ("Give every loopflow.studio page the same bar", "891d9a", "success", "1d", False, None),
    ]
    side = ""
    for title, ident, st, when, selected, ask in rows:
        cls = "row sel" if selected else "row"
        side += (f'<div class="{cls}"><b>{e(title)}</b><p><span class="mono id">{ident}</span> '
                 f'{status(st)} <span class="num">{when}</span></p>'
                 + (f'<p class="ask">{e(ask)}</p>' if ask else "") + "</div>")
    side += '<div class="row focus"><b>Waves</b><p>Tab lands here: the keyboard focus ring</p></div>'
    chips = "".join(f'<span class="chip {s}">{STATUS_WORD[s]}</span>' for s in STATUS + ["stopped"])
    term = (
        '<span class="c8"># a real run, colored by the 16 ANSI colors</span>\n'
        '<span class="c2">❯</span> cargo test <span class="c6">--lib</span>\n'
        '<span class="c3 b">warning</span>: unused variable: <span class="c4">`palette`</span>\n'
        '<span class="c1 b">error</span>[E0308]: mismatched types <span class="c5">--&gt;</span> src/skills/catalog.rs:41\n'
        'test skills::catalog::loads <span class="c2">ok</span>   test routes::login <span class="c1">FAILED</span>\n'
        '<span class="sel">selected text in the terminal</span> <span class="cur">▍</span>\n'
        '<span class="c9">9</span> <span class="c10">10</span> <span class="c11">11</span> <span class="c12">12</span> '
        '<span class="c13">13</span> <span class="c14">14</span> <span class="c15">15</span> '
        '<span class="c7">7</span> <span class="c0">0</span>'
    )
    return f"""<figure class="scr"><figcaption>Task workspace</figcaption>
<div class="win">
  <div class="tbar"><span class="t2">◧ ‹ ›</span> <b>loopflow</b> <span class="t2">/ product /</span> <b>50f7f9</b></div>
  <div class="cols">
    <div class="side"><p class="head">Working <span class="num">5</span></p>{side}</div>
    <div class="main">
      <p class="flow"><span class="mono">feature</span> explore · plan · <b class="run">build</b> · ship-review · deliver · verify</p>
      <p class="sent">build is running. You are next needed at ship-review.</p>
      <p class="chips">{chips}</p>
      <pre class="term">{term}</pre>
      <p class="acts"><button class="btn">Stop</button><button class="btn acc">Run</button><button class="btn acc h">Hover</button><button class="btn acc p">Pressed</button> <a>Open pull request</a> <span class="ter">tertiary</span></p>
    </div>
  </div>
</div></figure>"""


def diff(t):
    files = [("WorkPalette.swift", "swift/LoopflowMac/Design", 12, 4, True),
             ("ThemeFile.swift", "swift/Loopflow/Design", 31, 9, False),
             ("Loopflow Dark", "swift/Loopflow/Themes/Catalogue", 8, 8, False)]
    side = ""
    for name, where, add, rem, selected in files:
        side += (f'<div class="file{" on" if selected else ""}"><b>{name}</b><p>{where}</p>'
                 f'<p class="mono"><span class="add">+{add}</span> <span class="rem">−{rem}</span></p></div>')
    code = (
        '<p><i>41</i><s> </s>    public var selectionEdge: Color { textSecondary }</p>'
        '<p class="del"><i>42</i><s>−</s>    public var keyboardFocus: Color { <mark>accentInk</mark> }</p>'
        '<p class="add"><i>42</i><s>+</s>    public var keyboardFocus: Color { <mark>definition.focus.color</mark> }</p>'
        '<p><i>43</i><s> </s>    public var linkInk: Color { textSecondary }</p>'
        '<p class="pick"><i>44</i><s> </s>    public var linkHover: Color { text }</p>'
        '<p class="add"><i>45</i><s>+</s>    public var diffAdded: Color { successTone.fill }</p>'
        '<p class="del"><i>46</i><s>−</s>    public var actionHover: Color { surfaceElement }</p>'
        '<p><i>47</i><s> </s>    public static var light: Self { ThemeDefinition.light.palette }</p>'
    )
    return f"""<figure class="scr"><figcaption>Diff</figcaption>
<div class="win">
  <div class="tabs"><span>Brief</span><span class="on">Diff</span><span>Files</span><em>1 of 3 viewed</em></div>
  <div class="cols"><div class="side files">{side}</div><div class="main code">{code}</div></div>
</div></figure>"""


def chooser(t, names):
    listing = "".join(f'<span class="{"on" if n == t["name"] else ""}">{e(n)}</span>' for n in names)
    ladder = "".join(f'<span style="background:var(--{k})">{k.replace("surface-", "")}</span>'
                     for k in ["background", "surface", "surface-muted", "surface-element"])
    words = "".join(status(s) for s in STATUS)
    ansi = "".join(f'<span style="color:var(--a{i})">{"Aa"}</span>' for i in range(16))
    return f"""<figure class="scr"><figcaption>Choose Theme</figcaption>
<div class="win pick">
  <h4>Choose Theme</h4>
  <p class="seg"><span>Light</span><span>Dark</span><span class="on">System</span></p>
  <div class="cols">
    <div class="names">{listing}</div>
    <div class="sample">
      <div class="ladder">{ladder}</div>
      <p>Text <span class="sec">secondary</span> <span class="ter">tertiary</span> <span class="emp">emphasis</span> <span class="ai">accent ink</span></p>
      <p class="onmuted">{words}</p>
      <p class="onelement">{words}</p>
      <div class="ansi">{ansi}</div>
    </div>
  </div>
</div></figure>"""


def screens(t, names):
    return f'<div class="theme-screens" style="{css_vars(t)}">{workspace(t)}{diff(t)}{chooser(t, names)}</div>'


def failure_list(fs):
    if not fs:
        return '<p class="none">Nothing fails these measurements.</p>'
    out = ""
    for g in GROUPS:
        items = [(level, text) for group, level, text in fs if group == g]
        if not items:
            continue
        out += f"<h4>{g}</h4><ul>"
        for level, text in sorted(items, key=lambda x: ["fail", "close", "note"].index(x[0])):
            out += f'<li class="{level}"><b>{LEVEL_WORD[level]}.</b> {swatched(text)}</li>'
        out += "</ul>"
    return out


def swatched(text):
    out, i, text = "", 0, e(text)
    while i < len(text):
        if text[i] == "#" and i + 7 <= len(text) and all(c in "0123456789abcdef" for c in text[i + 1:i + 7]):
            out += f'<span class="sw" style="background:{text[i:i + 7]}"></span><code>{text[i:i + 7]}</code>'
            i += 7
        else:
            out += text[i]
            i += 1
    return out


def count(fs, level="fail"):
    return sum(1 for _, lv, _ in fs if lv == level)


def change_table(rows):
    body = "".join(
        f'<tr><td>{e(k)}</td><td>{swatched(a)}</td><td>{swatched(b)}</td></tr>' for k, a, b in rows)
    return f'<table class="chg"><thead><tr><th>Role</th><th>Now</th><th>Proposed</th></tr></thead><tbody>{body}</tbody></table>'


def variant(title, blurb, t, fs, names, base=None):
    rows = changes(base, t) if base else None
    left = failure_list(fs)
    return f"""<div class="variant"><h3>{title}</h3><p>{blurb}</p>
<div class="theme">{screens(t, names)}<div class="fails">{left}
{('<details><summary>' + str(len(rows)) + ' values change</summary>' + change_table(rows) + '</details>') if rows else ''}</div></div></div>"""


def build():
    themes = load()
    names = [t["name"] for t in themes]
    by = {t["name"]: t for t in themes}
    found = {t["name"]: findings(t) for t in themes}
    light, dark = by["Loopflow Light"], by["Loopflow Dark"]
    lr, dr, dp = light_repair(light), dark_repair(dark), dark_pair(dark, light)
    b1, b2, b3 = (dark_pair(dark, light, c) for c in (0.34, 0.67, 1.0))
    facts = pair_facts(light, dark)
    pf = pair_facts(light, dp)[1]

    # Summary table.
    rows = ""
    for t in themes:
        fs = found[t["name"]]
        cells = "".join(
            f'<td class="num">{sum(1 for g, lv, _ in fs if g == grp and lv == "fail") or "·"}</td>' for grp in GROUPS)
        rows += (f'<tr><td><a href="#{slug(t["name"])}">{e(t["name"])}</a></td><td>{"dark" if t["dark"] else "light"}</td>'
                 f'{cells}<td class="num">{count(fs, "close")}</td><td class="num">{t["derived_count"]} of 32</td></tr>')

    # Catalogue-wide counts for the opening.
    def n(pred):
        return sum(1 for t in themes if any(pred(g, lv, tx) for g, lv, tx in found[t["name"]]))
    same_tints = n(lambda g, lv, tx: g == "Diff" and lv == "fail" and "tints are the same" in tx)
    flat_sel = n(lambda g, lv, tx: "file selection fill is invisible" in tx)
    focus_sel = n(lambda g, lv, tx: "focus ring on the selected row" in tx)
    one_text = n(lambda g, lv, tx: "secondary and tertiary text are one color" in tx)
    ansi_low = n(lambda g, lv, tx: g == "Terminal" and lv == "fail")
    plain_emph = n(lambda g, lv, tx: "emphasis ink is the text color" in tx)
    mapped = sum(1 for t in themes if t["derived_count"] >= 20)
    tbar = n(lambda g, lv, tx: "title bar secondary" in tx)

    # Near-duplicates.
    pairs = []
    for i, a in enumerate(themes):
        for b in themes[i + 1:]:
            if a["dark"] == b["dark"]:
                app, term = theme_distance(a, b)
                pairs.append((app, term, a, b))
    dup_rows = ""
    for app, term, a, b in sorted(pairs, key=lambda p: min(p[0], p[1] + 1.5))[:10]:
        sw = lambda t: "".join(  # noqa: E731
            f'<span class="sw big" style="background:{t["roles"][k]}"></span>'
            for k in ["background", "surface", "text", "accent", "working-ink", "warning-ink", "failure-ink", "success-ink"])
        dup_rows += (f'<tr><td><a href="#{slug(a["name"])}">{e(a["name"])}</a><br>{sw(a)}</td>'
                     f'<td><a href="#{slug(b["name"])}">{e(b["name"])}</a><br>{sw(b)}</td>'
                     f'<td class="num">{app:.1f}</td><td class="num">{term:.1f}</td></tr>')

    sections = ""
    for t in themes:
        fs = found[t["name"]]
        sections += f"""<section id="{slug(t['name'])}"><h3>{e(t['name'])} <small>{'dark' if t['dark'] else 'light'} · {count(fs)} fail, {count(fs, 'close')} close</small></h3>
<div class="theme">{screens(t, names)}<div class="fails">{failure_list(fs)}</div></div></section>"""

    lf, df = facts
    page = f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Bundled themes, measured</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Old+Standard+TT&family=Atkinson+Hyperlegible+Next:wght@400;600&family=Spline+Sans+Mono:wght@400;700&display=swap" rel="stylesheet">
<style>{CSS}</style></head><body>
<main>
<h1>Bundled themes, measured</h1>
<p class="lede">All 24 themes in <code>swift/Loopflow/Themes/Catalogue/</code> on the same three screens, with what fails beside each. Nothing in the theme files has changed. Colors are read from the files, value for value; the title bar, selection and diff tints are composited the way the app paints them.</p>
<p>The screens are HTML built for this audit, not the app. They put every role where the app uses it: status words on the sidebar, the selected row, chips, the terminal's 16 colors, added and removed lines, a selected file.</p>
<nav><a href="#found">What the catalogue shows</a> · <a href="#defaults">The two defaults, before and after</a> · <a href="#all">All 24</a> · <a href="#count">Is 24 the right number</a> · <a href="#rex">What Rex does</a> · <a href="#how">How it was measured</a></nav>

<h2 id="found">What the catalogue shows</h2>
<p><code>WorkPaletteTests</code> holds in every theme: ordinary text reaches 4.5:1 on every surface and on the neutral selection over it. What it does not measure is where the failures are.</p>
<ol class="found">
<li><b>Added and removed lines look alike in {same_tints} of 24 themes.</b> The Diff paints a line with the status chip's fill, and the chip fills were made quiet: at most 7% of the status color over the background. In Loopflow Light the two tints are {swatched(light['roles']['success-fill'])} and {swatched(light['roles']['failure-fill'])}. The plus and minus carry the meaning alone.</li>
<li><b>A selected file has no fill in {flat_sel} of 24.</b> <code>selection-fill</code> is the theme's own <code>surface</code> color at 0.72, so over a surface it changes nothing. Only the 2 pt edge marks the file.</li>
<li><b>Keyboard focus disappears on the selected row in {focus_sel} themes.</b> The ring is <code>accent-ink</code>; on the Work selection fill it is under 3:1. DESIGN.md asks for focus that is visible on a selected row.</li>
<li><b>Secondary and tertiary text are one color in {one_text} themes</b>, Loopflow Dark among them. Each must reach 4.5:1 on the surface of the ladder furthest from the ground, and where that surface is mid-tone there is no room for a third level.</li>
<li><b>{mapped} of 24 themes are the import mapping's output</b> with an accent and a selection fill chosen by hand; three (Frost, Pewter, Night Owl) are the mapping untouched but for the shadow. Only the two defaults are authored role by role. In {plain_emph} themes emphasis ink is plain text, so headings and repo names carry no color.</li>
<li><b>{ansi_low} themes ship ANSI colors under 3:1</b> on their own terminal background. Loopflow sets Ghostty's <code>minimum-contrast = 3</code>, so the terminal shifts them and shows colors the theme file does not name.</li>
<li><b>Title bar secondary text is under 4.5:1 in {tbar} light themes.</b> It is the title bar text at 72% over the accent.</li>
<li><b>The active pane border is under 3:1 in both defaults</b> (Light {cr(light['roles']['border-active'], light['roles']['background']):.1f}:1, Dark {cr(dark['roles']['border-active'], dark['roles']['background']):.1f}:1 on the background). The other 22 take it from accent ink and pass.</li>
</ol>
<table class="sum"><thead><tr><th>Theme</th><th>Mode</th>{''.join(f'<th>{g}</th>' for g in GROUPS)}<th>Close</th><th>Roles the mapping derived</th></tr></thead><tbody>{rows}</tbody></table>
<p class="soft">Counts are failures. A dot is none. “Close” counts pairs that pass a threshold but sit near it.</p>

<h2 id="defaults">The two defaults, before and after</h2>
<h3>Are they one theme?</h3>
<p>They share a name, the burgundy button fill {swatched(lf['accent'])} and the status hues (within 6° of each other, except Available). They do not share a ground or an ink.</p>
<table class="sum"><thead><tr><th></th><th>Loopflow Light</th><th>Loopflow Dark</th></tr></thead><tbody>
<tr><td>Ground</td><td>{swatched(lf['ground'])} warm, hue {lf['ground_hue']:.0f}°</td><td>{swatched(df['ground'])} cool slate, hue {df['ground_hue']:.0f}°</td></tr>
<tr><td>Accent ink and emphasis ink (links you press, headings, repo names, focus)</td><td>{swatched(lf['accent_ink'])} burgundy</td><td>{swatched(df['accent_ink'])} bronze, {hue_gap(lf['accent_ink'], df['accent_ink']):.0f}° away</td></tr>
<tr><td>Title bar against the window</td><td>{swatched(lf['titlebar'])} {lf['titlebar_ratio']:.1f}:1</td><td>{swatched(df['titlebar'])} {df['titlebar_ratio']:.2f}:1, burgundy mixed 65% into the slate by <code>titlebarBackground</code></td></tr>
<tr><td>Accent button against the window</td><td>{lf['button_ratio']:.1f}:1</td><td>{df['button_ratio']:.2f}:1</td></tr>
<tr><td>Burgundy in the terminal</td><td>ANSI magenta is {swatched(light['ansi'][5])}</td><td>ANSI magenta is {swatched(dark['ansi'][5])}, a mauve</td></tr>
<tr><td>Available</td><td>{swatched(light['roles']['available-ink'])} burgundy family</td><td>{swatched(dark['roles']['available-ink'])} chroma {lch(dark['roles']['available-ink'])[1]:.3f}, ΔE {de(dark['roles']['available-ink'], dark['roles']['text-secondary']):.1f} from secondary text</td></tr>
</tbody></table>
<p>In Light the burgundy is the title bar, the headings, the focus ring and the button. In Dark it survives as a button fill at {df['button_ratio']:.2f}:1 and a title bar at {df['titlebar_ratio']:.2f}:1; everything that is burgundy ink in Light is bronze. Cover the name and Dark reads as a slate theme with warm text.</p>
<p>That leaves a choice only you can make: Dark stays the slate-and-bronze it has always been and gets repaired, or Dark becomes Light turned over. Both are drawn below. Light has one proposal, since it is the reference.</p>

<h3>Loopflow Light</h3>
{variant("Now", "As shipped.", light, found["Loopflow Light"], names)}
{variant("Proposed: repair", "The same theme. Diff and chip fills are strong enough to tell added from removed. A selected file gets a fill: the text color at 5%, which tints every surface equally. Hover and pressed differ. Bright ANSI colors are lighter, more saturated versions of the normal ones, still 4.5:1. The active pane border reaches 3:1. Status inks darken a step so they hold 4.5:1 on the new selection.", lr, findings(lr), names, light)}

<h3>Loopflow Dark</h3>
{variant("Now", "As shipped.", dark, found["Loopflow Dark"], names)}
{variant("Proposed A: repair, slate and bronze kept", "The identity it has today. The surface ladder is shallower, which makes room for a real tertiary text. Status inks get the chroma Light's have (about 0.09, up from 0.02 to 0.06), so Available is no longer grey. Fills, file selection, active border and ANSI contrast are fixed as in Light. The title bar and button still sink into the slate: that needs burgundy to change, or <code>titlebarBackground</code> to stop mixing 65% of the ground into it.", dr, findings(dr), names, dark)}
{variant("Proposed B: Light turned over", f"Light's ink {swatched(light['roles']['text'])} becomes the ground and Light's paper stays the text, so the two share one warm neutral. Burgundy comes back as ink: accent ink and emphasis ink are a rose at the burgundy hue ({pf['accent_ink_hue']:.0f}°, Light's is {lf['accent_ink_hue']:.0f}°). The button fill lifts one step to {swatched(dp['roles']['accent'])} with Light's burgundy as its hover. ANSI magenta is burgundy family, as in Light. The bronze goes. The title bar is still mixed into the ground by code ({pf['titlebar_ratio']:.2f}:1).", dp, findings(dp), names, dark)}
<h3 id="bluer">Dark B's red on a bluer grey</h3>
<p>The same burgundy ink, rose emphasis and status colors as B. Only the ground and the surfaces built on it move back toward today's slate {swatched(dark['roles']['background'])}. Three stops, so the amount of blue is the only thing to choose.</p>
{variant("B1: a third of the way to slate", "Still a warm charcoal at a glance; the blue shows beside Light's paper.", b1, findings(b1), names, dark)}
{variant("B2: two thirds", "A neutral blue-grey. The rose ink has the most room here: it is no longer warm on warm.", b2, findings(b2), names, dark)}
{variant("B3: today's slate, with B's ink", f"The ground Loopflow Dark ships now. Burgundy is furthest from the ground's hue, so the title bar mix is at its muddiest ({pair_facts(light, b3)[1]['titlebar_ratio']:.2f}:1, {swatched(pair_facts(light, b3)[1]['titlebar'])}).", b3, findings(b3), names, dark)}
<p><b>The choice:</b> Light's repair, yes or no; then which ground for Dark: B, B1, B2 or B3. No theme file changes until you say which.</p>

<h2 id="all">All 24</h2>
<p class="soft">Loopflow's own four first, then light companions, then dark.</p>
{sections}

<h2 id="count">Is 24 the right number</h2>
<p>Distance is the mean OKLab ΔE across twelve app roles, and across the 16 ANSI colors. Under about 3, two themes are hard to tell apart on the same screen.</p>
<table class="sum"><thead><tr><th>Theme</th><th>Theme</th><th>App roles ΔE</th><th>Terminal ΔE</th></tr></thead><tbody>{dup_rows}</tbody></table>
<ul>
<li><b>Four pale cool lights are one theme in four tints.</b> Celadon, Vellum, Sea Glass and Frost sit within ΔE 2 to 5 of each other. Celadon and Sea Glass have the same terminal colors, value for value.</li>
<li><b>Onyx and Phosphor</b> are ΔE 2.4 apart in the app and 2.1 in the terminal: a near-black with a green or a teal cast.</li>
<li><b>Linen and Hearth reuse the defaults' terminal palettes exactly</b> and differ from them in ground and accent only. They read as "Loopflow Light without the burgundy" and "Loopflow Dark, charcoal".</li>
<li><b>Moss and Rosewood</b> share a terminal palette (ΔE 2.8) on different grounds; they are told apart easily, by ground alone.</li>
<li><b>Three are untouched imports</b> (Frost, Pewter, Night Owl), which anyone gets by dropping the upstream Ghostty file into their themes folder.</li>
</ul>
<p>A catalogue of 16 would lose nothing a person can see: keep one or two of the four pale lights, one of Onyx and Phosphor, and decide whether Linen and Hearth are themes or the defaults' "quiet" setting. The names people look for by reputation (Catppuccin, Gruvbox, Flexoki, Solarized, Night Owl) are the ones with the most terminal failures, because their upstream palettes were tuned for other backgrounds or other contrast floors.</p>

<h2 id="rex">What Rex does</h2>
<p class="soft">Read from the Rex Beta app installed on this Mac (its bundled design tokens and its saved preferences). Structure and measured relationships only; no theme files, color values or code were taken. Its public docs were not consulted because the app itself was here.</p>
<table class="sum"><thead><tr><th></th><th>Rex</th><th>Loopflow</th></tr></thead><tbody>
<tr><td>Themes shipped</td><td>41: 14 light, 27 dark</td><td>24: 12 light, 12 dark</td></tr>
<tr><td>Defaults</td><td>Merino Light and Merino Dark, stored as a light theme and a dark theme that follow the system</td><td>Loopflow Light and Loopflow Dark, with a Light, Dark or System mode</td></tr>
<tr><td>What a theme is</td><td>A terminal source (background, foreground, cursor, selection, 16 ANSI) plus generation flags. All 73 interface roles are generated from it; no shipped theme overrides one.</td><td>A terminal source plus 32 app roles written in the file. 22 of 24 are mostly the mapping's output, kept as static text.</td></tr>
<tr><td>Roles beyond the terminal</td><td>Four surfaces and a separate terminal background; two strokes; control rest, hover and pressed as translucent paints; three text levels; an accent family of eleven (ink, solid, on-solid, container, on-container, border, line, dim, focus ring); four status families (danger, success, warning, info) of six each: ink, solid, on-solid, container, on-container, border; selection and its foreground.</td><td>Four surfaces; three borders; three text levels; emphasis ink; accent fill with hover and pressed, accent text, accent ink; Work selection fill and two texts; one file selection fill; six status tones of ink and fill.</td></tr>
<tr><td>Interface and terminal as one</td><td>Window ground equals terminal background in 39 of 41, text equals terminal foreground in all 41. Status ink is ANSI red, green, yellow, blue exactly in 28 of 41 and within 3° of hue in all. The solid accent is the terminal cursor in 30 of 41.</td><td>Ground equals terminal background in all 24. Status inks are ANSI colors with 40% of their chroma removed, so the interface is greyer than its terminal. The cursor is the foreground color; the accent appears in the terminal only in Loopflow Light, as ANSI magenta.</td></tr>
<tr><td>Light and dark defaults as a pair</td><td>One accent hue (217° light, 220° dark) and one neutral hue in both. Contrast steps run in parallel: secondary text 4.9 and 5.3, tertiary 3.3 and 3.6, accent 5.0 and 5.3.</td><td>Ground hue {lf['ground_hue']:.0f}° and {df['ground_hue']:.0f}°; accent ink {lf['accent_ink_hue']:.0f}° and {df['accent_ink_hue']:.0f}°.</td></tr>
<tr><td>Contrast floor</td><td>Tertiary text sits at about 3.5:1 (lowest 3.3). 58 of 574 chromatic ANSI colors are under 3:1.</td><td>Every text level at 4.5:1 on every surface, tested.</td></tr>
<tr><td>Color space</td><td>Display P3</td><td>sRGB</td></tr>
</tbody></table>
<h3>What Loopflow can take from it</h3>
<ol class="found">
<li><b>Generate, don't transcribe.</b> Rex's themes cannot drift from their generator because the roles are never written down per theme. Loopflow already has the generator (<code>GhosttyThemeMapping</code>), and 22 built-ins are its output frozen into files. Every catalogue-wide failure above (same diff tints, invisible file selection, focus on selection) is one rule in the mapping, repeated 22 times. Fixing the rule and regenerating fixes them all; hand-editing 22 files fixes them once.</li>
<li><b>A status color needs more than ink and one fill.</b> Rex separates the quiet container (a chip) from the solid and from the border. Loopflow asks one fill to be a quiet chip and a diff line, and it can only be one. A container role for diff lines, or a stronger shared fill, is the missing piece.</li>
<li><b>Selection that works on any surface is translucent.</b> Rex's control states are paints with alpha. Loopflow's file selection is an opaque surface color at a fixed opacity, which is why it vanishes on the surface it was copied from. The proposals above use the text color at 5% for this reason.</li>
<li><b>The pair shares its hues.</b> Rex's two defaults keep accent and neutral hue fixed and move only lightness. That is Dark B above.</li>
<li><b>Put the accent in the terminal.</b> Rex ties the accent to the cursor. A burgundy cursor would carry Loopflow's one color into the pane that is the product, in both defaults.</li>
<li><b>Where Loopflow is stricter, keep the reason in view.</b> Rex lets tertiary text fall to 3.3:1. Loopflow holds 4.5:1, and the price is the collapsed tertiary in five themes. Either the ladder gets shallower (Dark A and B do this) or tertiary is allowed a lower floor for text nobody must read. That is an open question, not a finding.</li>
</ol>

<h2 id="how">How it was measured</h2>
<ul>
<li>Contrast is WCAG 2 relative luminance, as in <code>WorkPaletteTests</code>: 4.5:1 for text, 3:1 for marks (focus ring, active border, the plus and minus).</li>
<li>Likeness is distance in OKLab ×100. Under 3 is called the same; under 5, close.</li>
<li>“Roles the mapping derived” ports <code>GhosttyThemeMapping.palette</code> and counts app roles in the file that equal what an import of the same terminal colors would get.</li>
<li>Terminal: every ANSI color against the theme's terminal background, leaving out the color that is the ground by convention (black on dark, white on light).</li>
<li>Not measured here: the Dev app itself. The brief's last check, both defaults in the Dev app, comes after a choice.</li>
<li>Rebuild: <code>python3 -I scratch/theme-audit/build.py</code>.</li>
</ul>
</main></body></html>"""
    (HERE / "index.html").write_text(page)
    print(f"wrote {HERE / 'index.html'} ({len(page) // 1024} KB)")


CSS = """
:root { --serif: "Old Standard TT", Georgia, serif; --sans: "Atkinson Hyperlegible Next", -apple-system, system-ui, sans-serif; --mono: "Spline Sans Mono", ui-monospace, Menlo, monospace; }
* { box-sizing: border-box; }
body { margin: 0; background: #fff; color: #1a1a1a; font: 400 15px/1.5 var(--sans); }
main { max-width: 1680px; margin: 0 auto; padding: 32px 24px 96px; }
h1 { font: 400 40px/1.1 var(--serif); letter-spacing: -0.03em; font-variant-ligatures: no-common-ligatures; margin: 0 0 12px; }
h2 { font: 400 28px/1.15 var(--serif); letter-spacing: -0.03em; font-variant-ligatures: no-common-ligatures; margin: 64px 0 12px; padding-top: 16px; border-top: 1px solid #e5e5e5; }
h3 { font: 600 17px/1.3 var(--sans); margin: 32px 0 8px; }
h3 small { font: 400 13px var(--sans); color: #555; margin-left: 8px; }
main > p, main > ul, main > ol, .variant > p { max-width: 78ch; }
.lede { font-size: 17px; }
.soft { color: #555; font-size: 13px; }
nav { margin: 16px 0; font-size: 14px; }
a { color: #722f37; }
code { font: 400 0.88em var(--mono); }
ol.found li, main > ul li { margin-bottom: 8px; }
table.sum, table.chg { border-collapse: collapse; font-size: 13px; margin: 12px 0; }
table.sum th, table.sum td, table.chg th, table.chg td { text-align: left; padding: 4px 12px 4px 0; border-bottom: 1px solid #e5e5e5; vertical-align: top; }
table.sum th, table.chg th { font-weight: 600; }
.num { font-variant-numeric: tabular-nums; }
td.num { text-align: right; }
.sw { display: inline-block; width: 11px; height: 11px; border-radius: 2px; border: 1px solid rgba(128,128,128,.45); vertical-align: -1px; margin-right: 3px; }
.sw.big { width: 18px; height: 18px; margin: 2px 2px 0 0; }
section { scroll-margin-top: 8px; }
.theme { display: grid; grid-template-columns: minmax(0, 1fr) 360px; gap: 20px; align-items: start; margin-bottom: 8px; }
.theme-screens { display: grid; grid-template-columns: repeat(auto-fit, minmax(400px, 1fr)); gap: 12px; }
.fails { font-size: 13px; line-height: 1.4; }
.fails h4 { font: 600 13px var(--sans); margin: 10px 0 2px; }
.fails h4:first-child { margin-top: 0; }
.fails ul { margin: 0; padding-left: 16px; }
.fails li { margin-bottom: 4px; }
.fails li.close, .fails li.note { color: #555; }
.fails li.fail b { color: #a02e25; }
.fails .none { color: #555; }
details { margin-top: 10px; } summary { cursor: pointer; font-weight: 600; }
.variant { margin: 12px 0 28px; }
.variant h3 { margin-bottom: 2px; }
@media (max-width: 1100px) { .theme { grid-template-columns: 1fr; } }

/* The three screens. Every color below is a theme role. */
figure.scr { margin: 0; min-width: 0; }
figure.scr figcaption { font-size: 12px; color: #555; margin-bottom: 4px; }
.win { background: var(--background); color: var(--text); border: 1px solid rgba(128,128,128,.4); border-radius: 12px; overflow: hidden; font: 400 13px/1.35 var(--sans); height: 476px; display: flex; flex-direction: column; }
.win b { font-weight: 600; }
.win p { margin: 0; }
.mono { font-family: var(--mono); font-size: 12px; }
.tbar { background: var(--titlebar); color: var(--titlebar-text); padding: 6px 12px; flex: none; }
.tbar .t2 { color: var(--titlebar-2); }
.cols { display: flex; flex: 1; min-height: 0; }
.side { width: 44%; background: var(--surface); border-right: 1px solid var(--border); padding: 8px; overflow: hidden; }
.side .head { color: var(--text-tertiary); font-weight: 600; padding: 0 8px 4px; }
.row { padding: 5px 8px; border-radius: 8px; border: 1px solid transparent; }
.row b { font-weight: 400; display: block; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.row p { font-size: 11px; color: var(--text-secondary); }
.row .id { color: var(--text-secondary); font-size: 11px; }
.row.sel { background: var(--work-selected-fill); border-color: var(--text-secondary); }
.row.sel b { color: var(--work-selected-text); font-weight: 600; white-space: normal; }
.row.sel p, .row.sel .id { color: var(--work-selected-secondary-text); }
.row.sel .st { color: var(--work-selected-text); font-weight: 600; }
.row.sel .ask { font-size: 13px; }
.row.focus { outline: 2px solid var(--accent-ink); outline-offset: -2px; margin-top: 4px; }
.st i { display: none; }
.st.working { color: var(--working-ink); } .st.warning { color: var(--warning-ink); } .st.available { color: var(--available-ink); }
.st.failure { color: var(--failure-ink); } .st.success { color: var(--success-ink); }
.main { flex: 1; min-width: 0; padding: 8px 10px; display: flex; flex-direction: column; gap: 6px; }
.flow { color: var(--text-secondary); font-size: 12px; } .flow .run { color: var(--working-ink); }
.sent { color: var(--text-secondary); font-size: 12px; }
.chips { display: flex; flex-wrap: wrap; gap: 4px; }
.chip { font-weight: 600; font-size: 11px; padding: 1px 6px; border-radius: 4px; }
.chip.working { color: var(--working-ink); background: var(--working-fill); } .chip.warning { color: var(--warning-ink); background: var(--warning-fill); }
.chip.available { color: var(--available-ink); background: var(--available-fill); } .chip.failure { color: var(--failure-ink); background: var(--failure-fill); }
.chip.success { color: var(--success-ink); background: var(--success-fill); } .chip.stopped { color: var(--stopped-ink); background: var(--stopped-fill); }
.term { flex: 1; margin: 0; background: var(--term-bg); color: var(--term-fg); border: 1px solid var(--border-active); border-radius: 8px; padding: 6px 8px; font: 400 11px/1.5 var(--mono); white-space: pre-wrap; overflow: hidden; }
.term .b { font-weight: 700; }
""" + "".join(f".term .c{i} {{ color: var(--a{i}); }} " for i in range(16)) + """
.term .sel { background: var(--term-sel-bg); color: var(--term-sel-fg); }
.term .cur { color: var(--cursor); }
.acts { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; font-size: 12px; }
.acts a { color: var(--text-secondary); text-decoration: underline; } .ter { color: var(--text-tertiary); }
.btn { font: 400 12px var(--sans); height: 22px; padding: 0 8px; border-radius: 4px; border: 1px solid var(--border-strong); background: var(--surface-element); color: var(--text); }
.btn.acc { background: var(--accent); color: var(--accent-text); border-color: transparent; }
.btn.acc.h { background: var(--accent-hover); } .btn.acc.p { background: var(--accent-pressed); }
.tabs { display: flex; gap: 12px; padding: 6px 12px; border-bottom: 1px solid var(--border); color: var(--text-secondary); flex: none; }
.tabs .on { color: var(--text); font-weight: 600; } .tabs em { margin-left: auto; font-style: normal; color: var(--text-tertiary); font-size: 11px; }
.files { width: 34%; }
.file { padding: 5px 8px; border-radius: 8px; position: relative; }
.file p { font-size: 11px; color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.file.on { background: color-mix(in srgb, var(--selection-fill) var(--sel-op), transparent); }
.file.on::before { content: ""; position: absolute; left: 0; top: 4px; bottom: 4px; width: 2px; border-radius: 1px; background: var(--text-secondary); }
.add { color: var(--success-ink); } .rem { color: var(--failure-ink); }
.code { padding: 6px 0; gap: 0; font: 400 11px/1.7 var(--mono); overflow: hidden; }
.code p { white-space: pre; padding-right: 8px; }
.code i { font-style: normal; display: inline-block; width: 26px; text-align: right; color: var(--text-tertiary); }
.code s { text-decoration: none; display: inline-block; width: 18px; text-align: center; color: var(--text-tertiary); }
.code p.add { background: var(--success-fill); color: var(--text); } .code p.del { background: var(--failure-fill); color: var(--text); }
.code p.add s { color: color-mix(in srgb, var(--success-ink) 80%, transparent); } .code p.del s { color: color-mix(in srgb, var(--failure-ink) 80%, transparent); }
.code p.add mark { background: color-mix(in srgb, var(--success-ink) 26%, transparent); color: inherit; }
.code p.del mark { background: color-mix(in srgb, var(--failure-ink) 26%, transparent); color: inherit; }
.code p.pick { background: color-mix(in srgb, var(--accent) 16%, transparent); }
.win.pick { padding: 12px; gap: 8px; }
.win.pick h4 { margin: 0; font: 400 26px/1 var(--serif); letter-spacing: -0.03em; font-variant-ligatures: no-common-ligatures; color: var(--emphasis-ink); }
.seg { display: inline-flex; align-self: flex-start; border: 1px solid var(--border-strong); border-radius: 4px; overflow: hidden; font-size: 12px; }
.seg span { padding: 2px 10px; color: var(--text-secondary); } .seg .on { background: var(--surface-element); color: var(--text); font-weight: 600; }
.names { width: 40%; columns: 2; column-gap: 4px; font-size: 10.5px; line-height: 1.25; }
.names span { display: block; padding: 2px 5px; border-radius: 4px; color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.names .on { background: var(--work-selected-fill); color: var(--work-selected-text); font-weight: 600; }
.sample { flex: 1; min-width: 0; padding-left: 10px; display: flex; flex-direction: column; gap: 6px; font-size: 12px; }
.ladder { display: flex; border: 1px solid var(--border); border-radius: 8px; overflow: hidden; }
.ladder span { flex: 1; padding: 6px 4px; font-size: 10.5px; color: var(--text-secondary); border-right: 1px solid var(--border); }
.sec { color: var(--text-secondary); } .emp { color: var(--emphasis-ink); } .ai { color: var(--accent-ink); }
.onmuted, .onelement { padding: 4px 6px; border-radius: 4px; display: flex; gap: 8px; flex-wrap: wrap; }
.onmuted { background: var(--surface-muted); } .onelement { background: var(--surface-element); }
.ansi { background: var(--term-bg); border-radius: 8px; padding: 4px 6px; display: grid; grid-template-columns: repeat(8, 1fr); font: 400 12px/1.5 var(--mono); }
"""

if __name__ == "__main__":
    build()
