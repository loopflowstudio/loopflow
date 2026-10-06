import re, json, sys, html
src, out, title = sys.argv[1], sys.argv[2], sys.argv[3]
txt = open(src).read().split("\n")
start = next(i for i, l in enumerate(txt) if "Thread_" in l and "main" in l)
end = next(i for i, l in enumerate(txt) if i > start and re.match(r"\s+\d+ Thread_", l))
def demangle(name):
    if not name.startswith("_R"): return name
    s, i, parts = name, 2, []
    while i < len(s):
        if s.startswith("Cs", i):
            j = s.find("_", i)
            i = j + 1 if j > 0 else i + 2
            continue
        m = re.match(r"(\d+)_?", s[i:])
        if m and (i == 0 or not s[i-1].isdigit()):
            n = int(m.group(1)); a = i + len(m.group(0)); seg = s[a:a+n]
            if 2 <= n == len(seg) and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", seg):
                parts.append(seg); i = a + n; continue
        i += 1
    drop = {"core","future","ops","function","closure","alloc","std","loopflow","commands","lf"}
    keep = [p for p in parts if p not in drop][:5]
    return "::".join(keep) if keep else name[:40]
root = {"n": "main thread", "v": 0, "c": []}
stack = [(-1, root)]
for l in txt[start:end]:
    m = re.match(r"^(\s+[+!:| ]*)(\d+) (.+?)(?:  \(in ([^)]+)\))?(?: \+ [\d,]+)?(?:  \[0x[0-9a-f,]+\])?\s*$", l)
    if not m: continue
    d = len(m.group(1)); n = int(m.group(2)); name = m.group(3).strip(); lib = m.group(4) or ""
    if "Thread_" in name: root["v"] = n; continue
    node = {"n": demangle(name.split("  (in")[0]), "v": n, "c": [], "l": lib}
    while stack[-1][0] >= d: stack.pop()
    stack[-1][1]["c"].append(node); stack.append((d, node))
def squash(node):
    # collapse runtime plumbing: single-child chains of scheduler/trait frames
    noise = re.compile(r"tokio|block_on|enter_runtime|BlockingRegion|CachedParkThread|call_once|lang_start|__rust_begin|^start$|^main$|GenericShunt|try_process|spec_from_iter|spec_extend|extend_desugared|from_iter|MappedRows|AndThenRows|FallibleStreamingIterator|try_fold|try_for_each|Iterator|^sqlite3[A-Z]|^sqlite3Vdbe|^sqlite3Btree|^pcache|^pager|^btree|^vdbe|unixRead|seekAndRead|getPageNormal|readDbPage|moveToChild|moveToLeftmost")
    for c in node["c"]: squash(c)
    changed = True
    while changed:
        changed = False
        new = []
        for c in node["c"]:
            if noise.search(c["n"]) and c["c"]:
                new.extend(c["c"]); changed = True
            else: new.append(c)
        node["c"] = new
    # merge same-named siblings
    merged = {}
    for c in node["c"]:
        if c["n"] in merged:
            merged[c["n"]]["v"] += c["v"]; merged[c["n"]]["c"].extend(c["c"])
        else: merged[c["n"]] = c
    node["c"] = sorted(merged.values(), key=lambda c: -c["v"])
    if len(merged) != len(new if False else node["c"]): pass
squash(root); squash(root)
def prune(node, floor):
    node["c"] = [c for c in node["c"] if c["v"] >= floor]
    for c in node["c"]: prune(c, floor)
prune(root, max(3, root["v"] // 400))
data = json.dumps(root, separators=(",", ":"))
page = """<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Cold reading flame chart</title><style>
:root{--ink:#1f1b18;--soft:#6b635c;--paper:#fbf8f3;--rule:#e4ddd3}
body{margin:0;background:var(--paper);color:var(--ink);font:14px/1.5 -apple-system,BlinkMacSystemFont,Helvetica,Arial,sans-serif}
header{padding:20px 20px 8px;max-width:1100px}h1{font:600 22px/1.2 Georgia,serif;margin:0 0 6px}p{margin:0 0 6px;color:var(--soft);max-width:80ch}
#bar{padding:6px 20px;font:12px ui-monospace,Menlo,monospace;min-height:20px;border-top:1px solid var(--rule);border-bottom:1px solid var(--rule);background:#fff;position:sticky;top:0;z-index:2;overflow-wrap:anywhere}
#chart{padding:12px 20px 40px;overflow-x:auto}svg{display:block}rect{stroke:var(--paper);stroke-width:1;cursor:pointer}rect:hover{stroke:#000}
text{font:11px ui-monospace,Menlo,monospace;pointer-events:none;fill:#1f1b18}
.k{display:inline-block;width:10px;height:10px;margin:0 4px 0 12px;vertical-align:-1px}button{font:12px inherit;margin-left:12px}
</style></head><body><header><h1>__TITLE__</h1>
<p>Main thread of <code>lf monitor workspace --watch</code>, sampled every 1 ms from process start. Width is wall time, including time blocked on a child process; depth is the call stack, callers on top. Stacks are merged, so left-to-right order is by cost, not by time. Click a frame to zoom; click the top bar to reset.</p>
<p><span class="k" style="background:#e9a23b"></span>Git subprocess<span class="k" style="background:#6fa8dc"></span>SQLite<span class="k" style="background:#b7a3d6"></span>process admission and store open<span class="k" style="background:#9cc59c"></span>process activity (ps)<span class="k" style="background:#d9d2c6"></span>idle wait<span class="k" style="background:#e8c9b8"></span>other</p></header>
<div id="bar">hover a frame</div><div id="chart"></div>
<script>
const root=__DATA__;const H=20;let focus=root;
function color(n,inh){const s=n.n;
 if(/retained_output|engine::git|git_common_dir|worktree_root|is_clean|rev_parse|is_ancestor|posix_spawn|waitpid|wait4/.test(s))return'#e9a23b';
 if(/sqlite|open_execs|members|store::|Statement|rusqlite/.test(s))return'#6fa8dc';
 if(/admit_process|journal::emit|try_emit|exec_attribution|resolve_managed_wave|migrations|validate_foreign|lifecycle|ulock|pthread_join/.test(s))return'#b7a3d6';
 if(/load_snapshot|observe_processes|sample_processes|parse_processes|top::/.test(s))return'#9cc59c';
 if(/Condvar|wait_timeout|psynch_cvwait|park/.test(s))return'#d9d2c6';
 return inh||'#e8c9b8';}
function depth(n){return 1+Math.max(0,...n.c.map(depth));}
function draw(){const W=Math.max(900,document.getElementById('chart').clientWidth-40);const D=depth(focus);
 let out=[];function rec(n,x,w,y,inh){if(w<0.8)return;const c=color(n,inh);
  const ms=n.v, pct=(100*n.v/root.v).toFixed(1);
  out.push(`<rect x="${x.toFixed(1)}" y="${y}" width="${w.toFixed(1)}" height="${H-1}" fill="${c}" data-n="${encodeURIComponent(n.n)}" data-v="${ms}" data-p="${pct}"></rect>`);
  if(w>40){const max=Math.floor((w-6)/6.6);let t=n.n+' '+ms+' ms';if(t.length>max)t=t.slice(0,Math.max(0,max-1))+'…';out.push(`<text x="${(x+3).toFixed(1)}" y="${y+14}">${t.replace(/&/g,'&amp;').replace(/</g,'&lt;')}</text>`);}
  let cx=x;const inherit=(c==='#e8c9b8')?inh:c;for(const k of n.c){const kw=w*k.v/n.v;rec(k,cx,kw,y+H,inherit);cx+=kw;}}
 rec(focus,0,W,0,null);
 document.getElementById('chart').innerHTML=`<svg width="${W}" height="${D*H+4}" role="img" aria-label="flame chart">${out.join('')}</svg>`;
 const idx=new Map();(function walk(n,p){idx.set(n,p);n.c.forEach(k=>walk(k,n));})(root,null);
 document.querySelectorAll('rect').forEach(r=>{r.onmouseenter=()=>{document.getElementById('bar').textContent=decodeURIComponent(r.dataset.n)+' — '+r.dataset.v+' ms ('+r.dataset.p+'% of the window)';};
  r.onclick=()=>{const name=decodeURIComponent(r.dataset.n),v=+r.dataset.v;let hit=null;(function f(n){if(hit)return;if(n.n===name&&n.v===v){hit=n;return;}n.c.forEach(f);})(focus);focus=(hit&&hit!==focus)?hit:root;draw();};});}
draw();addEventListener('resize',draw);
</script></body></html>"""
open(out, "w").write(page.replace("__DATA__", data).replace("__TITLE__", html.escape(title)))
print("root ms", root["v"], "top:", [(c["n"][:50], c["v"]) for c in root["c"][:6]])
