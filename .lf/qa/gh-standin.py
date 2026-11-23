#!/usr/bin/python3 -I
# Stateful `gh` stand-in for .lf/qa/stacked-tasks.md: PRs live in $GH_QA_STATE, calls log to $GH_QA_STATE.log.
import json, os, subprocess, sys
S = os.environ["GH_QA_STATE"]
args = sys.argv[1:]
open(S + ".log", "a").write(" ".join(args) + "\n")
if args[:1] == ["--version"]: print("gh version 2.0 (qa)"); sys.exit(0)
if args[:2] == ["auth", "status"]: sys.exit(0)
try: st = json.load(open(S))
except Exception: st = {"next": 100, "prs": []}
def save(): json.dump(st, open(S, "w"), indent=1)
def opt(name, default=None):
    return args[args.index(name) + 1] if name in args else default
def branch(): return subprocess.run(["git", "branch", "--show-current"], capture_output=True, text=True).stdout.strip()
def head(b): return subprocess.run(["git", "rev-parse", "origin/" + b], capture_output=True, text=True).stdout.strip() or None
def view(pr, fields):
    out = {"number": pr["number"], "url": "https://github.com/qa/repo/pull/%d" % pr["number"], "state": pr["state"],
           "isDraft": pr["draft"], "headRefOid": head(pr["head"]), "baseRefName": pr["base"], "headRefName": pr["head"],
           "title": pr["title"], "body": pr["body"], "mergeCommit": None, "mergedAt": None}
    return {k: out.get(k) for k in fields.split(",")} if fields else out
def find(sel):
    for pr in st["prs"]:
        if str(pr["number"]) == sel or pr["head"] == sel: return pr
sub = " ".join(args[:2])
if sub == "pr list":
    h = opt("--head"); state = opt("--state", "open").upper()
    prs = [p for p in st["prs"] if (h is None or p["head"] == h) and (state == "ALL" or p["state"] == state)]
    print(json.dumps([view(p, opt("--json")) for p in prs]))
elif sub == "pr create":
    b = opt("--head", branch())
    pr = {"number": st["next"], "head": b, "base": opt("--base", "main"), "title": opt("--title"), "body": opt("--body"),
          "draft": "--draft" in args, "state": "OPEN"}
    st["next"] += 1; st["prs"].append(pr); save()
    print("https://github.com/qa/repo/pull/%d" % pr["number"])
elif sub == "pr edit" and "--base" in args and os.path.exists(S + ".fail-edit"):
    print("GraphQL: Resource not accessible by integration (updatePullRequest)", file=sys.stderr); sys.exit(1)
elif sub == "pr edit":
    pr = find(args[2])
    for k, f in (("title", "--title"), ("body", "--body"), ("base", "--base")):
        if f in args: pr[k] = opt(f)
    save(); print("https://github.com/qa/repo/pull/%d" % pr["number"])
elif sub == "pr ready":
    find(args[2])["draft"] = False; save()
elif sub == "pr view":
    sel = args[2] if len(args) > 2 and not args[2].startswith("-") else branch()
    pr = find(sel)
    if pr is None: print("no pull requests found", file=sys.stderr); sys.exit(1)
    if "-q" in args:
        print(view(pr, "")[opt("-q").lstrip(".")])
    elif "--json" in args: print(json.dumps(view(pr, opt("--json"))))
    else: print(view(pr, "")["url"])
elif args[0] == "api" and any("/pulls/" in a for a in args):
    n = [a for a in args if "/pulls/" in a][0].rsplit("/", 1)[1]
    pr = find(n)
    if pr is None: print("HTTP 404: Not Found", file=sys.stderr); sys.exit(1)
    print(json.dumps({"number": pr["number"], "state": "closed" if pr["state"] != "OPEN" else "open",
        "merged": pr["state"] == "MERGED", "draft": pr["draft"], "merge_commit_sha": pr.get("merge_commit"),
        "merged_at": "2026-11-18T16:00:00Z" if pr["state"] == "MERGED" else None, "mergeable_state": "clean",
        "html_url": "https://github.com/qa/repo/pull/%d" % pr["number"], "head": {"sha": pr.get("merged_head") or head(pr["head"])},
        "base": {"ref": pr["base"]}}))
elif sub == "api graphql":
    import re
    q = [a for a in args if a.startswith("query=")][0]
    repo = {}
    for alias, h in re.findall(r'(b\d+): pullRequests\(first: \d+, headRefName: "([^"]+)"', q):
        repo[alias] = {"nodes": [{"number": p["number"], "headRefOid": head(p["head"]), "state": p["state"]} for p in st["prs"] if p["head"] == h]}
    for alias, ref in re.findall(r'(r\d+): ref\(qualifiedName: "refs/heads/([^"]+)"\)', q):
        repo[alias] = {"id": "ref-" + ref} if head(ref) else None
    print(json.dumps({"data": {"repository": repo}}))
else:
    print("qa gh: unhandled " + " ".join(args), file=sys.stderr); sys.exit(1)
