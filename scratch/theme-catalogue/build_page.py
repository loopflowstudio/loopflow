import json, sys
from pathlib import Path

S = Path(sys.argv[1])
rows = {r["name"]: r for r in json.load(open(S / "themes.json"))}

BAD = "MIT, Mark Badolato (author's own theme page)"
UP = " Ship upstream colors; sample shows the catalogue port."

# (display name, upstream file, status, why, license note)
proposed = [
    ("Catppuccin Latte", "Catppuccin Latte", "keep", "Cool pastel light; pairs with Mocha.", "MIT (Catppuccin; official Ghostty port)"),
    ("Gruvbox Light", "Gruvbox Light", "keep", "Retro cream light; pairs with Gruvbox Dark.", "MIT/X11 (morhetz README and package.json)"),
    ("Flexoki Light", "Flexoki Light", "add", "Warm ink-on-paper light; strong text contrast.", "MIT (Steph Ango; official terminal ports)"),
    ("Noctis Lilac", "Noctis Lilac", "add", "Lilac-tinted light; the only pink-violet light." + UP, "MIT (Liviu Schera)"),
    ("Manila", "Yngwie's Strat", "renamed", "Yellow legal-pad light; strongest tint of any light.", BAD),
    ("Clay", "Desert Morning", "renamed", "Tan mid-tone light; lowest glare of the lights.", BAD),
    ("Pewter", "Nvim Light", "renamed", "Cool gray light, dimmer than white.", "Apache-2.0 (Neovim default palette)"),
    ("Frost", "Icy Glacier", "renamed", "Blue-tinted white; 6.4:1 ANSI.", BAD),
    ("Vellum", "Paper Tiger", "renamed", "Warm white with 7.7:1 ANSI; brighter than Flexoki.", BAD),
    ("Celadon", "Safety Glass", "renamed", "Mint-tinted white; the only green-blue light.", BAD),
    ("Catppuccin Mocha", "Catppuccin Mocha", "keep", "Popular pastel violet dark.", "MIT (Catppuccin; official Ghostty port)"),
    ("Gruvbox Dark", "Gruvbox Dark", "keep", "Warm retro brown-gray dark.", "MIT/X11 (morhetz README and package.json)"),
    ("Solarized Dark", "iTerm2 Solarized Dark", "keep", "Teal classic; the original, now without the iTerm2 name. Text 4.8:1 clears AA.", "MIT (Ethan Schoonover; official iTerm2 colors)"),
    ("Flexoki Dark", "Flexoki Dark", "add", "Warm near-black ink; pairs with Flexoki Light.", "MIT (Steph Ango; official terminal ports)"),
    ("Night Owl", "Night Owl", "add", "Deep navy; the one saturated blue." + UP, "MIT (Sarah Drasner)"),
    ("Onyx", "Black Box Recorder", "renamed", "True black; 8.3:1 ANSI, the most readable dark.", BAD),
    ("Rosewood", "Desert Rose", "renamed", "Rose-tinted dark; echoes Loopflow burgundy.", BAD),
    ("Aubergine", "Witching Hour", "renamed", "Saturated purple dark; deeper than Mocha.", BAD),
    ("Moss", "Cholla", "renamed", "Olive mid-dark; the only yellow-green dark.", BAD),
    ("Phosphor", "Green Screen", "renamed", "Green phosphor retro dark.", BAD),
]

leaving = [
    ("Alabaster", "Plain white; duplicates other whites."),
    ("Atom One Light", "ANSI colors fall to 1.9:1; Atom is a GitHub product name."),
    ("Atom One Dark", "Fourth blue-violet dark; product name."),
    ("Everforest Light Med", "Text 4.7:1, ANSI 1.8:1."),
    ("Everforest Dark Med", "Blue-gray dark already covered; ANSI hues drift."),
    ("GitHub Light Default", "Trademark name; plain white."),
    ("GitHub Dark Default", "Trademark name; blue-violet cluster."),
    ("Rose Pine Dawn", "ANSI green/blue map to other hues; status colors mislead."),
    ("Rose Pine Moon", "Same hue mapping problem; violet cluster."),
    ("Rose Pine", "Same hue mapping problem; violet cluster."),
    ("TokyoNight Day", "Text 4.5:1."),
    ("TokyoNight Storm", "Violet cluster; near-twin of TokyoNight."),
    ("TokyoNight", "Violet cluster; on the bench."),
    ("Dracula", "Mocha holds the violet slot; on the bench."),
    ("Nord", "Overlaps Loopflow Dark slate; on the bench."),
]

excluded = [
    ("Modus Operandi", "GPL-3.0 upstream (protesilaos/modus-themes)."),
    ("Modus Vivendi", "GPL-3.0 upstream. Onyx takes the true-black slot."),
    ("Monokai SublimeText", "Original Monokai has no license; this port is LicenseRef-Unknown."),
    ("Man Page", "Apple Terminal profile colors; LicenseRef-Unknown. Manila now comes from Yngwie's Strat."),
    ("Ubuntu", "Canonical console colors via a Super User answer; LicenseRef-Unknown."),
    ("Solarized Dark Higher Contrast", "Unlicensed gist variant; ship the MIT original instead."),
]

bench = [
    ("Tokyo Night", "TokyoNight", "Popular indigo; Apache-2.0."),
    ("Dracula", "Dracula", "Popular vivid violet; MIT."),
    ("Nord", "Nord", "Popular muted blue-gray; MIT. Overlaps Loopflow Dark."),
    ("Kanagawa", "Kanagawa Wave", "Ink-wash muted indigo; MIT."),
    ("Mulberry", "Desert Bloom", "Plum-magenta dark; Badolato MIT."),
    ("Quartz", "Afterimage", "Pink-tinted white, 7.3:1 ANSI; Badolato MIT."),
    ("Walnut", "Jute Espresso", "Coffee mid-tone dark; overlaps Hearth. Badolato MIT."),
]


THEMES = Path(sys.argv[2])


def _hx(v):
    return "#%06x" % v["hex"]


def _rgb(h):
    return tuple(int(h[i:i + 2], 16) / 255 for i in (1, 3, 5))


def _lum(h):
    f = lambda c: c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = map(f, _rgb(h))
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def _cr(a, b):
    x, y = sorted((_lum(a), _lum(b)), reverse=True)
    return (x + 0.05) / (y + 0.05)


def loopflow(file, why):
    d = json.load(open(THEMES / file))
    t = d["terminal"]
    bg, fg = _hx(t["background"]), _hx(t["foreground"])
    ansi = [_hx(v) for v in t["indexed"][:16]]
    return {"name": d["name"], "upstream": d["name"], "status": "loopflow", "why": why, "bg": bg, "fg": fg,
            "ansi": ansi, "dark": _lum(bg) < 0.18, "fgc": round(_cr(fg, bg), 2),
            "ansic": round(min(_cr(c, bg) for c in ansi[1:7]), 2), "L": _lum(bg) ** (1 / 3), "hue": 0, "chroma": 0,
            "license": "Loopflow"}


def theme(file, **extra):
    r = rows[file]
    return {"bg": r["bg"], "fg": r["fg"], "ansi": r["ansi"], "dark": r["dark"],
            "fgc": r["fg_contrast"], "ansic": r["ansi_min_contrast"],
            "hue": r["bgH"], "chroma": r["bgC"], "L": r["bgL"], "upstream": file, **extra}


current_names = ["Alabaster", "Atom One Light", "Catppuccin Latte", "Everforest Light Med", "GitHub Light Default",
                 "Gruvbox Light", "Rose Pine Dawn", "TokyoNight Day", "Atom One Dark", "Catppuccin Mocha", "Dracula",
                 "Everforest Dark Med", "GitHub Dark Default", "Gruvbox Dark", "iTerm2 Solarized Dark", "Nord",
                 "Rose Pine Moon", "Rose Pine", "TokyoNight Storm", "TokyoNight"]

lf4 = [loopflow("Loopflow Light.json", "Original cream and burgundy."), loopflow("Linen.json", "Quiet warm neutral with green."),
       loopflow("Loopflow Dark.json", "Original slate with bronze."), loopflow("Hearth.json", "Warm charcoal.")]
data = {
    "loopflow": lf4,
    "proposed": [theme(f, name=n, status=s, why=w, license=l) for n, f, s, w, l in proposed],
    "leaving": [theme(f, name=f, why=w) for f, w in leaving],
    "bench": [theme(f, name=n, why=w) for n, f, w in bench],
    "excluded": [theme(f, name=f, why=w) for f, w in excluded],
    "current": [theme(f, name=f) for f in current_names],
}
assert sum(not t["dark"] for t in data["proposed"]) == 10 and len(data["proposed"]) == 20 and len(set(t["name"] for t in data["proposed"])) == 20

template = (S / "tools" / "page.html").read_text()
Path(sys.argv[3]).write_text(template.replace("/*DATA*/null", json.dumps(data)))
print("ok", sum(t["dark"] for t in data["proposed"]), "dark of 20")
