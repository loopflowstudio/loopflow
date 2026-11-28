#!/usr/bin/python3 -I
# Stand-in claude for terminal-link QA (.lf/qa/desktop.md, "Terminal links"): stays alive, and whenever
# $MK/print appears, clears the screen and prints each "label<TAB>target" line of $MK/links.txt as an
# OSC 8 link ({cwd} becomes the checkout; label PLAIN prints bare text). Line 1 shows TERM_PROGRAM.
import os, sys, time
MK = os.environ.get("LINK_QA_MK", "/tmp/lf-qa-links/mk")
if sys.argv[1:2] == ["--version"]:
    print("2.1.332 (Claude Code)"); sys.exit(0)
try:
    if not sys.stdin.isatty(): sys.stdin.read()
except Exception: pass
out = open("/dev/tty", "w")
def show():
    cwd = os.getcwd()
    out.write("\x1b[2J\x1b[H")
    out.write("TERM_PROGRAM=%s TERM_PROGRAM_VERSION=%s\r\n" % (os.environ.get("TERM_PROGRAM"), os.environ.get("TERM_PROGRAM_VERSION")))
    for line in open(os.path.join(MK, "links.txt")):
        line = line.rstrip("\n")
        if not line: continue
        label, target = line.split("\t", 1)
        target = target.replace("{cwd}", cwd)
        if label == "PLAIN":
            out.write("%-6s %s\r\n" % (label, target))
        else:
            out.write("%-6s \x1b]8;;%s\x1b\\%s\x1b]8;;\x1b\\\r\n" % (label, target, target))
    out.flush()
show()
while True:
    p = os.path.join(MK, "print")
    if os.path.exists(p):
        os.remove(p); show()
    time.sleep(0.5)
