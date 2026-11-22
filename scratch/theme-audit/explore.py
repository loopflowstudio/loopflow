"""Write explore.html: what people love across apps, the catalogue after this round, and a proposal.

    python3 -I scratch/theme-audit/explore.py
"""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from audit import STATUS, STATUS_WORD, findings, lch, load, theme_distance  # noqa: E402
from build import CSS, css_vars, diff, e, failure_list, workspace  # noqa: E402
from families import FAMILIES, catalogue  # noqa: E402

# Read 2026-11-22. Numbers are what each source reported that day.
VSCODE = [("GitHub Theme", 20_693_379), ("One Dark Pro", 12_943_874), ("Dracula", 11_168_336),
          ("Atom One Dark", 7_427_458), ("Material Theme", 4_302_640), ("Ayu", 4_252_401), ("Monokai Pro", 4_203_992),
          ("Winter is Coming", 3_777_206), ("Night Owl", 3_617_100), ("Tokyo Night", 3_037_759),
          ("Palenight", 2_670_200), ("SynthWave '84", 2_576_397), ("Shades of Purple", 2_364_218),
          ("Cobalt2", 1_914_729), ("Andromeda", 1_715_475), ("Catppuccin", 1_501_346), ("Atom One Light", 1_464_543),
          ("Noctis", 1_403_169), ("Nord", 1_306_594), ("Gruvbox", 1_093_297)]
JETBRAINS = [("Material Theme UI", 19_053_412), ("One Dark", 11_841_648), ("Gerry Themes", 3_832_055),
             ("Dracula", 3_633_594), ("Monokai Pro", 2_535_969), ("Solarized", 2_468_014), ("Catppuccin", 2_264_605),
             ("Dark Purple", 2_004_630), ("Gradianto", 1_975_896), ("Xcode-Dark", 1_185_255), ("Nord", 840_830),
             ("GitHub", 671_353)]
SUBLIME = [("Material Theme", 1_371_188), ("1337", 1_129_647), ("Soda", 922_830), ("Ayu", 548_706),
           ("Spacegray", 519_538), ("Brogrammer", 483_153), ("Predawn", 412_300), ("Dracula", 272_583),
           ("Tomorrow", 178_866), ("Base16", 171_024)]
NEOVIM = ["Catppuccin", "Tokyo Night", "Kanagawa", "Rosé Pine", "Nightfox", "One Dark", "Gruvbox Material",
          "GitHub", "Everforest", "Cyberdream"]
STARS = [("Dracula", 23_616, "dracula/dracula-theme"), ("Catppuccin", 19_972, "catppuccin/catppuccin"),
         ("Solarized", 16_021, "altercation/solarized"), ("Gruvbox", 15_845, "morhetz/gruvbox"),
         ("Material", 11_302, "material-theme/vsc-material-theme"), ("Tokyo Night", 8_280, "folke/tokyonight.nvim"),
         ("Nord", 6_924, "nordtheme/nord"), ("Kanagawa", 6_456, "rebelot/kanagawa.nvim"),
         ("Everforest", 4_286, "sainnhe/everforest"), ("Nightfox", 4_103, "EdenEast/nightfox.nvim"),
         ("Flexoki", 3_859, "kepano/flexoki"), ("Rosé Pine", 3_112, "rose-pine/neovim"),
         ("Night Owl", 2_960, "sdras/night-owl-vscode-theme")]
PORTS = [("Dracula", 524, "dracula"), ("Catppuccin", 362, "catppuccin"), ("Rosé Pine", 160, "rose-pine"),
         ("Nord", 37, "nordtheme")]
SOURCES = {
    "vscode": "https://marketplace.visualstudio.com/search?target=VSCode&category=Themes&sortBy=Installs",
    "jetbrains": "https://plugins.jetbrains.com/search?tags=Theme&orderBy=downloads",
    "sublime": "https://packagecontrol.io/browse/labels/color%20scheme",
    "neovim": "https://dotfyle.com/neovim/colorscheme/top",
}


def bars(rows, top):
    out = ""
    for name, n in rows:
        out += (f'<tr><td>{e(name)}</td><td class="bar"><span style="width:{100 * n / top:.1f}%"></span></td>'
                f'<td class="num">{n / 1e6:.1f}M</td></tr>')
    return f'<table class="bars">{out}</table>'


def card(t, side, fails):
    r = t["roles"]
    dots = "".join(f'<i style="background:{t["ansi"][i]}"></i>' for i in (1, 2, 3, 4, 5, 6))
    words = "".join(f'<span style="color:{r[s + "-ink"]}">{STATUS_WORD[s]}</span>' for s in ("working", "warning", "failure"))
    note = f"{fails} fail" if fails else "passes"
    return (f'<button class="card" type="button" data-side="{side}" data-name="{e(t["name"])}" '
            f'style="--bg:{r["background"]};--fg:{r["text"]};--fg2:{r["text-secondary"]};--bar:{r.get("titlebar") or "#000"};'
            f'--acc:{r["accent"]};--acct:{r["accent-text"]};--ink:{r["emphasis-ink"]};--sel:{r["work-selected-fill"]};--selt:{r["work-selected-text"]}">'
            f'<span class="cbar"></span><span class="cbody"><b>{e(t["name"])}</b>'
            f'<span class="cw">{words}</span><span class="dots">{dots}</span>'
            f'<span class="crow"><span class="csel">Selected row</span><span class="cbtn">Run</span></span></span>'
            f'<span class="cnote">{note}</span></button>')


def scatter(now, proposed):
    """Grounds on one plane: hue around, lightness up. Hollow is now, filled is proposed."""
    W, H, pad = 760, 260, 30
    out = []
    for t, filled in [(x, False) for x in now] + [(x, True) for x in proposed]:
        L, C, h = lch(t["roles"]["background"])
        hue = h if C > 0.006 else None
        x = pad + (hue / 360 * (W - 2 * pad - 60) if hue is not None else (W - 2 * pad - 30)) + (0 if hue is not None else 30)
        y = H - pad - L * (H - 2 * pad)
        fill = t["roles"]["background"] if filled else "none"
        stroke = t["roles"]["accent"] if filled else "currentColor"
        out.append(f'<circle cx="{x:.0f}" cy="{y:.0f}" r="{7 if filled else 5}" fill="{fill}" stroke="{stroke}" '
                   f'stroke-width="2"><title>{e(t["name"])}{" (proposed)" if filled else ""}</title></circle>')
    axis = "".join(f'<text x="{pad + h / 360 * (W - 2 * pad - 60):.0f}" y="{H - 8}">{lbl}</text>'
                   for h, lbl in [(30, "red"), (90, "yellow"), (150, "green"), (210, "cyan"), (270, "blue"), (330, "violet")])
    return (f'<svg class="scatter" viewBox="0 0 {W} {H}" role="img" aria-label="Each theme\'s background by hue and lightness">'
            f'<line x1="{pad}" y1="{H - pad}" x2="{W - pad}" y2="{H - pad}"/><line x1="{pad}" y1="{pad}" x2="{pad}" y2="{H - pad}"/>'
            f'{axis}<text x="{W - pad - 40}" y="{H - 8}">grey</text><text x="{pad + 6}" y="{pad + 4}">light</text>'
            f'<text x="{pad + 6}" y="{H - pad - 6}">dark</text>{"".join(out)}</svg>')


def build():
    now = load()
    proposed = catalogue()
    loopflow = [t for t in now if t["name"] in ("Loopflow Light", "Loopflow Dark")]
    for t in loopflow:
        t["family"] = "Loopflow"
    shown = loopflow + proposed
    found = {("A", t["name"]): findings(t) for t in now}
    found.update({("B", t["name"]): findings(t) for t in shown})
    fails = {k: sum(1 for f in v if f[1] == "fail") for k, v in found.items()}

    data = {}
    for side, ts in (("A", now), ("B", shown)):
        for t in ts:
            data[f"{side}:{t['name']}"] = {"vars": css_vars(t), "fails": failure_list(
                [f for f in found[(side, t["name"])] if f[1] != "note"])}

    names_now = [t["name"] for t in now]
    names_new = [t["name"] for t in shown]

    def row(side, label, first, names):
        listing = "".join(f'<span>{e(n)}</span>' for n in names)
        ansi = "".join(f'<span style="color:var(--a{i})">Aa</span>' for i in range(16))
        words = "".join(f'<span class="st {s}">{STATUS_WORD[s]}</span>' for s in STATUS)
        chooser = f"""<figure class="scr"><figcaption>Choose Theme</figcaption><div class="win pick"><h4>Choose Theme</h4>
<p class="seg"><span>Light</span><span>Dark</span><span class="on">System</span></p><div class="cols"><div class="names">{listing}</div>
<div class="sample"><div class="ladder">{''.join(f'<span style="background:var(--{k})">{k.replace("surface-", "")}</span>' for k in ["background", "surface", "surface-muted", "surface-element"])}</div>
<p>Text <span class="sec">secondary</span> <span class="ter">tertiary</span> <span class="emp">emphasis</span> <span class="ai">accent ink</span></p>
<p class="onmuted">{words}</p><p class="onelement">{words}</p><div class="ansi">{ansi}</div></div></div></div></figure>"""
        return f"""<div class="viewer" id="row{side}"><h3><span class="which">{label}</span>: <span data-title></span></h3>
<div class="theme"><div class="theme-screens">{workspace(first)}{diff(first)}{chooser}</div><div class="fails" data-fails></div></div></div>"""

    grid_now = "".join(card(t, "A", fails[("A", t["name"])]) for t in now)
    grid_new = ""
    for fam in ["Loopflow"] + [f["family"] for f in FAMILIES]:
        members = [t for t in shown if t["family"] == fam]
        info = next((f for f in FAMILIES if f["family"] == fam), None)
        where = (f"Ground held by {e(info['answers'])}." if info and info.get("answers") else e(info["gap"]) if info else "")
        about = (f"<p><b>{fam}</b>: {e(info['idea'])} <span class='soft'>{where}</span></p>"
                 if info else "<p><b>Loopflow</b>: the pair as this round ships them, unchanged.</p>")
        grid_new += f'<div class="fam">{about}<div class="grid">{"".join(card(t, "B", fails[("B", t["name"])]) for t in members)}</div></div>'

    # Where each current theme lands in the proposal.
    cover = ""
    for t in now:
        same = [p for p in shown if p["dark"] == t["dark"]]
        best = min(same, key=lambda p: theme_distance(t, p)[0])
        d = theme_distance(t, best)[0]
        cover += (f'<tr><td>{e(t["name"])}</td><td>{e(best["name"])}</td><td class="num">{d:.1f}</td>'
                  f'<td>{"close stand-in" if d < 8 else "same territory" if d < 14 else "not covered"}</td></tr>')

    now_fail = sum(fails[("A", t["name"])] for t in now)
    new_fail = sum(fails[("B", t["name"])] for t in proposed)
    top = max(n for _, n in VSCODE)

    page = f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Loved themes and a proposal</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Old+Standard+TT&family=Atkinson+Hyperlegible+Next:wght@400;600&family=Spline+Sans+Mono:wght@400;700&display=swap" rel="stylesheet">
<style>{CSS}{PAGE_CSS}</style></head><body>
<main>
<h1>Loved themes, and a catalogue built on them</h1>
<p class="lede">What people pick across editors and terminals, the 24 Loopflow themes as this round leaves them, and a proposal: {len(shown)} themes in {len(shown) // 2} light-and-dark families, generated from one rule set so every one passes the same measurements.</p>
<nav><a href="#answer">The answer</a> · <a href="#evidence">Evidence</a> · <a href="#map">Map</a> · <a href="#compare">Compare</a> · <a href="#cover">Coverage</a> · <a href="#limits">Limits</a> · <a href="#questions">Questions</a> · <a href="index.html">The audit</a></nav>

<h2 id="answer">The answer</h2>
<ol class="found">
<li><b>Two generations.</b> By total installs the incumbents still lead: GitHub, One Dark, Dracula, Material and Monokai top VS Code and JetBrains. Among people who build their own setup (Neovim configs, GitHub stars, ports) the current favorites are soft pastel families: Catppuccin, Tokyo Night, Kanagawa, Rosé Pine.</li>
<li><b>Loved themes are families, not files.</b> Catppuccin ships four flavors, Tokyo Night four styles, Rosé Pine three; Solarized, Flexoki and GitHub are one palette in light and dark. You choose the family once and the appearance follows the system.</li>
<li><b>They follow you between apps.</b> Dracula's organization has 524 repositories of ports and Catppuccin's 362. A terminal-first app is somewhere people expect their theme to show up already.</li>
<li><b>Almost everything loved is dark.</b> GitHub Light, Atom One Light and Solarized Light are the only light themes in reach of the top lists.</li>
<li><b>The grounds cluster into a handful of territories</b>: neutral slate (One Dark), navy (Night Owl, Material, Cobalt2), violet-navy (Catppuccin, Rosé Pine, Tokyo Night, Dracula), warm roast (Gruvbox, Kanagawa, Everforest), blue-grey (Nord), neon purple (SynthWave '84, Shades of Purple), paper and ink (Solarized, Flexoki, GitHub), and near-monochrome (Min, zenbones).</li>
</ol>
<p>The catalogue after this round covers four of those territories well and some twice over (four pale cool lights), and none of slate-with-vivid-syntax or neon. The proposal gives each territory one family in Loopflow's own values, keeps the Loopflow pair exactly as this round ships it, and adds a high-contrast pair. Three more families go where nothing loved is yet, the empty stretches of the map: Verdigris (green-cyan), Fig (magenta, with its hues swapped between light and dark) and Terracotta (red-orange). After this round, the 24 themes have <b>{now_fail}</b> failing measurements between them; the {len(proposed)} generated themes have <b>{new_fail}</b>.</p>

<h2 id="evidence">Evidence</h2>
<div class="ev">
<div><h3>VS Code Marketplace, installs</h3>{bars(VSCODE, top)}<p class="soft">Color themes from the <a href="{SOURCES['vscode']}">Themes category sorted by installs</a>; icon packs and language bundles left out.</p></div>
<div><h3>JetBrains Marketplace, downloads</h3>{bars(JETBRAINS, top)}<p class="soft"><a href="{SOURCES['jetbrains']}">Theme tag sorted by downloads</a>.</p>
<h3>Sublime Text, unique installs</h3>{bars(SUBLIME, top)}<p class="soft"><a href="{SOURCES['sublime']}">Package Control</a>, color scheme and theme labels. An older audience.</p></div>
<div><h3>Neovim, most-used in published configs</h3><ol class="plain">{''.join(f'<li>{e(n)}</li>' for n in NEOVIM)}</ol><p class="soft"><a href="{SOURCES['neovim']}">dotfyle</a>, rank only.</p>
<h3>GitHub stars</h3><table class="sum">{''.join(f'<tr><td><a href="https://github.com/{r}">{e(n)}</a></td><td class="num">{s / 1000:.1f}k</td></tr>' for n, s, r in STARS)}</table>
<h3>Ports to other apps</h3><table class="sum">{''.join(f'<tr><td><a href="https://github.com/{o}">{e(n)}</a></td><td class="num">{p} repositories</td></tr>' for n, p, o in PORTS)}</table>
<p class="soft">Rex, measured in the audit: its two defaults are GitHub's palette.</p></div>
</div>

<h2 id="map">Map</h2>
<p>Every theme's background, by hue (left to right) and lightness (up). Hollow circles are the 24 after this round; filled circles are the {len(shown)} proposed, ringed in their accent. Hover for names.</p>
{scatter(now, shown)}

<h2 id="compare">Compare</h2>
<p>The top row is a theme after this round; the bottom row is one from the proposal. Click any card below to put it in its row. The screens are the audit's HTML drawings, not the app. Failures beside each come from the same measurements.</p>
{row("A", "After this round", now[0], names_now)}
{row("B", "Proposed", shown[0], names_new)}
<div class="grids">
<div><h3>After this round: 24</h3><div class="grid">{grid_now}</div></div>
<div><h3>Proposed: {len(shown)} in {len(shown) // 2} families</h3>{grid_new}</div>
</div>

<h2 id="cover">Coverage</h2>
<p>For each theme after this round, the nearest proposed theme of the same appearance (mean OKLab ΔE across twelve app roles). Under 8 is a close stand-in; under 14, the same territory.</p>
<table class="sum"><thead><tr><th>After this round</th><th>Nearest proposed</th><th>ΔE</th><th></th></tr></thead><tbody>{cover}</tbody></table>

<h2 id="limits">Limits</h2>
<ul>
<li>Install counts measure reach over a decade, not love today. GitHub Theme leads VS Code partly because it is the theme of GitHub; Material's lead in JetBrains and Sublime is from its peak years.</li>
<li>Neovim, stars and ports measure people who configure things, which is close to Loopflow's audience but not the same.</li>
<li>The families' values are generated from short specs in <code>families.py</code>; nothing is copied from any theme above. They answer the territory a theme holds, not the theme. Someone who wants Catppuccin wants Catppuccin, and Sorbet is not that.</li>
<li>Passing the measurements is not looking good. The generator holds contrast and separation; whether Arcade is fun or garish is for eyes.</li>
<li>A generated theme can only be as good as the generator's rules. This page uses a generator outside the app; the app's <code>GhosttyThemeMapping</code> has different rules.</li>
</ul>

<h2 id="questions">Questions</h2>
<ol class="qs">
<li>What should the catalogue be?
  <ol type="a"><li>Loopflow's own families, as proposed: original values, every one passing.</li>
  <li>The loved names bundled (Catppuccin, Tokyo Night, Dracula, One Dark, Gruvbox, Nord, Rosé Pine, GitHub; most are MIT), plus the Loopflow pair.</li>
  <li>Both: the Loopflow pair and a few families, with the loved names one click away through import.</li></ol></li>
<li>Should a theme be a family?
  <ol type="a"><li>Yes: you pick Tannin, and light or dark follows the system.</li>
  <li>No: keep two independent pickers, as Choose Theme has now.</li></ol></li>
<li>Should built-ins be generated from a spec, the way this page's are?
  <ol type="a"><li>Yes, in the app: a spec of a dozen numbers per theme and one generator, replacing 33 stated roles.</li>
  <li>Generate in a script and keep writing full files, as this round did for the defaults.</li>
  <li>No: hand-authored files.</li></ol></li>
<li>Which proposed families are worth keeping? Name them, or "none".</li>
<li>Where does this go?
  <ol type="a"><li>A new Task after this one lands.</li>
  <li>Into this Task now (it is at pull request; this would send it back to build).</li>
  <li>Nowhere yet.</li></ol></li>
</ol>
</main>
<script>
var DATA = {json.dumps(data)};
function show(side, name) {{
  var d = DATA[side + ":" + name]; if (!d) return;
  var row = document.getElementById("row" + side);
  row.setAttribute("style", d.vars);
  row.querySelector("[data-title]").textContent = name;
  row.querySelector("[data-fails]").innerHTML = d.fails;
  row.querySelectorAll(".names span").forEach(function (s) {{ s.className = s.textContent === name ? "on" : ""; }});
  document.querySelectorAll('.card[data-side="' + side + '"]').forEach(function (c) {{ c.setAttribute("aria-pressed", c.dataset.name === name); }});
}}
document.querySelectorAll(".card").forEach(function (c) {{
  c.addEventListener("click", function () {{ show(c.dataset.side, c.dataset.name); document.getElementById("compare").scrollIntoView({{block: "start"}}); }});
}});
show("A", {json.dumps(now[0]["name"])}); show("B", "Sorbet");
</script>
</body></html>"""
    (HERE / "explore.html").write_text(page)
    print(f"wrote {HERE / 'explore.html'} ({len(page) // 1024} KB); now fails {now_fail}, proposed fails {new_fail}")


PAGE_CSS = """
:root { --page: #ffffff; --ink-page: #1a1a1a; --soft: #555; --rule: #e5e5e5; --link: #722f37; }
@media (prefers-color-scheme: dark) { :root { --page: #1d1b1a; --ink-page: #eeeae4; --soft: #b2aca5; --rule: #3a3633; --link: #e6a4ab; } }
body { background: var(--page); color: var(--ink-page); }
h2 { border-top-color: var(--rule); }
a { color: var(--link); }
.soft, h3 small, figure.scr figcaption, .fails li.close, .fails li.note { color: var(--soft); }
table.sum th, table.sum td, table.chg th, table.chg td { border-bottom-color: var(--rule); }
.ev { display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 24px; }
table.bars { border-collapse: collapse; width: 100%; font-size: 13px; }
table.bars td { padding: 2px 8px 2px 0; white-space: nowrap; }
table.bars td.bar { width: 60%; }
table.bars td.bar span { display: block; height: 10px; border-radius: 2px; background: var(--link); }
ol.plain { margin: 0; padding-left: 20px; font-size: 14px; }
svg.scatter { width: 100%; max-width: 900px; color: var(--ink-page); font: 400 12px var(--sans); }
svg.scatter line { stroke: var(--rule); }
svg.scatter text { fill: var(--soft); }
.viewer h3 .which { color: var(--soft); font-weight: 400; }
.grids { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1.3fr); gap: 32px; margin-top: 24px; }
@media (max-width: 1000px) { .grids { grid-template-columns: 1fr; } }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 8px; }
.fam > p { margin: 14px 0 6px; font-size: 14px; }
.card { all: unset; box-sizing: border-box; cursor: pointer; display: flex; flex-direction: column; border-radius: 8px; overflow: hidden; border: 1px solid rgba(128,128,128,.35); background: var(--bg); color: var(--fg); font: 400 12px/1.3 var(--sans); }
.card:focus-visible { outline: 2px solid var(--link); outline-offset: 2px; }
.card[aria-pressed="true"] { outline: 2px solid var(--link); outline-offset: 2px; }
.cbar { height: 10px; background: var(--bar); }
.cbody { display: flex; flex-direction: column; gap: 4px; padding: 6px 8px 4px; }
.cbody b { font-weight: 600; color: var(--ink); }
.cw { display: flex; gap: 6px; font-size: 11px; }
.dots { display: flex; gap: 3px; } .dots i { width: 10px; height: 10px; border-radius: 50%; }
.crow { display: flex; gap: 4px; align-items: center; }
.csel { flex: 1; background: var(--sel); color: var(--selt); border-radius: 4px; padding: 1px 4px; font-size: 11px; }
.cbtn { background: var(--acc); color: var(--acct); border-radius: 4px; padding: 1px 6px; font-size: 11px; }
.cnote { font-size: 11px; color: var(--fg2); padding: 0 8px 6px; }
.viewer .theme { grid-template-columns: 1fr; }
.viewer .theme-screens { grid-template-columns: repeat(3, minmax(0, 1fr)); }
.viewer .fails { columns: 2 360px; } .viewer .fails h4 { break-after: avoid; }
@media (max-width: 1100px) { .viewer .theme-screens { grid-template-columns: 1fr; } }
ol.qs > li { margin-bottom: 10px; } ol.qs ol { margin-top: 4px; }
"""

if __name__ == "__main__":
    build()
