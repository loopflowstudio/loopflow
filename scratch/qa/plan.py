# plan.py <db> <traced.json> <session-id>: plan and time each traced boundary
# statement on a store copy, unanalyzed and then analyzed. Writes roll back.
# The traced statements come from boundary_statements_never_scan_tables_that_grow_with_use
# (dump TRACED to JSON from that test). Run with python3 -I.
import json, re, sqlite3, sys, time

GROWING = {"steers", "steer_batches", "steer_deliveries", "session_steers", "session_events", "tasks"}
db, traced, session = sys.argv[1:4]
statements = [s for s in dict.fromkeys(json.load(open(traced)))
              if not re.match(r"\s*(BEGIN|COMMIT|ROLLBACK|SAVEPOINT|RELEASE)\b", s, re.I)]
conn = sqlite3.connect(db, isolation_level=None)


def aliases(sql):
    words = [w for w in re.split(r"[^A-Za-z0-9_]+", sql) if w]
    names = set(GROWING)
    for at, word in enumerate(words):
        if word in GROWING and at + 1 < len(words):
            names.add(words[at + 2] if words[at + 1] == "AS" and at + 2 < len(words) else words[at + 1])
    return names


def run(label):
    scans, rows = [], []
    for sql in statements:
        sql = sql.replace("'session'", f"'{session}'")
        details = [r[3] for r in conn.execute("EXPLAIN QUERY PLAN " + sql)]
        names = aliases(sql)
        for d in details:
            if d.startswith("SCAN ") and d.split()[1] in names:
                scans.append((d, sql))
        conn.execute("SAVEPOINT qa")
        start = time.perf_counter()
        try:
            conn.execute(sql).fetchall()
        except sqlite3.Error as e:
            details.append(f"error: {e}")
        ms = (time.perf_counter() - start) * 1000
        conn.execute("ROLLBACK TO qa")
        conn.execute("RELEASE qa")
        rows.append((ms, " ".join(sql.split())[:110]))
    print(f"== {label}: {len(statements)} statements, {len(scans)} scans of growing tables")
    for d, sql in scans:
        print("  SCAN:", d, "in", " ".join(sql.split())[:160])
    for ms, sql in sorted(rows, reverse=True)[:6]:
        print(f"  {ms:7.3f} ms  {sql}")
    print(f"  total {sum(ms for ms, _ in rows):.2f} ms")


run("unanalyzed")
conn.execute("ANALYZE")
run("analyzed")
