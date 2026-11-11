"""One self-contained ledger: decisions, cohorts and every individual trial."""
import html
import json
from pathlib import Path

OUT=Path(__file__).parent
r=json.loads((OUT/'comparison-results.json').read_text())
esc=lambda value:html.escape(str(value))
def metric(v):return f"{v['median_ms']:.1f} [{v['range_ms'][0]:.1f}–{v['range_ms'][1]:.1f}]"
page=['''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>LOO-822 measurement ledger</title>
<style>body{max-width:1200px;margin:40px auto;padding:0 24px;font:15px/1.5 system-ui;color:#20242a}table{border-collapse:collapse;width:100%;margin:20px 0}td,th{border-bottom:1px solid #ccd2da;padding:9px;text-align:left;vertical-align:top}th{background:#f2f4f7}code{font-size:13px;overflow-wrap:anywhere}.keep{color:#17623c}.reject{color:#8b341b}details{margin:20px 0}.scroll{overflow:auto}</style>
<h1>Session-list optimization measurements</h1>
<p><strong class="keep">Better: keep d9e3c1f3b.</strong> Default CLI: 579.0 → 436.3 ms median (24.7% lower). Served: 775.3 → 210.7 ms (72.8% lower). Five alternating pairs each, non-overlapping ranges, matching full JSON, zero command/validation failures.</p>
<p><strong class="reject">No measurable additional benefit: reject f8b742d8c.</strong> Its source changes are restored to d9e3c1f3b. All incremental timing ranges overlap. The original 18–75 second stall remains inconclusive.</p>
<table><thead><tr><th>Commit</th><th>Value</th><th>Verdict</th><th>Hypothesis</th><th>Limiter</th></tr></thead><tbody>
<tr><td><code>00851d9c6</code></td><td>Original baseline CLI 788 [701–863] ms, n=5. Live-writer 1742 [832–2778] versus quiet 1145 [1008–3088] ms.</td><td>Historical baseline retained; original stall inconclusive.</td><td>Task scans or writer contention explain the delay.</td><td>Repeated Task scans dominate SQL. No reproduced long writer wait; provider workload ended incomplete.</td></tr>
<tr><td><code>d9e3c1f3b</code></td><td>CLI 436.3 [421.9–443.0] versus 579.0 [561.7–586.6] ms. Served 210.7 [196.2–217.2] versus 775.3 [747.3–885.5] ms. n=5 per side.</td><td class="keep">Kept: better.</td><td>Indexed binding, checkout-ancestor and primary-Session probes remove repeated Task scans; target 15% CLI and 25% served.</td><td>Fixed real store and warm reads; shared Machine and ambient process facts. Q102 fullscans fall 196,961 → 4,129 (CLI) and 762,015 → 4,400 (served); VM work falls 95.7% / 96.5%. Remaining wave-address work and external reads still cost time.</td></tr>
<tr><td><code>f8b742d8c</code></td><td>CLI 420.2 [410.5–429.5] versus 439.7 [420.7–461.3] ms. Served 198.1 [179.5–234.9] versus 204.4 [190.2–209.6] ms. n=5 per side.</td><td class="reject">Rejected: no measurable additional benefit.</td><td>Provider-thread/activity indexes and repository-scoped Task reads improve the remaining list and planning work.</td><td>All incremental ranges overlap, including board and Work selection. Q111 and Q114 reach zero fullscans; scoped Task reads occupy only 1.5–3.6 ms of the traced path. Fewer SQL steps are not an observed wall-time gain.</td></tr>
</tbody></table>
<p>All times below are milliseconds, median [minimum–maximum]; five pairs per comparison with order reversed each pair. One excluded warm-up precedes every timed result. CLI returns 100 Sessions; served returns 371. Network-denied Task status measures local behavior, not successful planning exchange. The profiler runs separately. Some ambient AgentProcesses exited before those runs: all three trace builds match at 9/37 turns rather than the timed 11/52. Trace whole-path times cannot replace the timing cohort.</p>
<h2>Complete cohorts</h2><div class="scroll"><table><thead><tr><th>Comparison</th><th>Read</th><th>Side</th><th>Median [range] ms</th><th>Rows</th><th>n / errors</th></tr></thead><tbody>''']
all_rows=[]
for comparison,workloads in r['cohorts'].items():
 for workload,sides in workloads.items():
  for side,v in sides.items():
   page.append(f'<tr><td>{esc(comparison)}</td><td>{esc(workload)}</td><td>{esc(side)}</td><td>{metric(v)}</td><td>{v["rows"]}</td><td>{v["n"]} / {v["errors"]}</td></tr>')
   all_rows.extend((s,'retained measurement') for s in v['samples'])
page.append('</tbody></table></div><h2>Excluded work and limitations</h2><p>One preflight board-validator error was fixed before timing. The first online Task-status cohort was stopped after one baseline result; the candidate result is missing. Replacement planning runs deny network access, and blocked exchange attempts were not counted as successful work. Initial trace logs had interleaved records and frozen elapsed times; they are excluded from attribution. Shared-Machine load varied; no artificial load was added.</p><p>Frozen snapshot: 14,731 Sessions, 236,270 events, 12,955 activity rows, 160,214 Processes and 1,099 Tasks. Application rows match across stores; only migration ledgers differ. Full JSON is compared without dropping timestamps or paths. Ambient OS/Git facts and legacy absolute transcript paths remain limitations.</p>')
all_rows.extend((s,'excluded: setup, calibration, abandoned cohort or initial collector') for s in r['other_trials'])
page.append('<details><summary>Every individual trial, including excluded trials</summary><div class="scroll"><table><thead><tr><th>Trial</th><th>ms</th><th>Warm-up ms</th><th>Rows</th><th>Errors</th><th>Full JSON digest</th><th>Disposition</th></tr></thead><tbody>')
for s,disposition in all_rows:
 value=f'{s["ms"]:.3f}' if 'ms' in s else 'missing'
 warm=f'{s["warmup"]["ms"]:.3f}' if 'warmup' in s else 'not recorded'
 page.append(f'<tr><td>{esc(s["label"])}</td><td>{value}</td><td>{warm}</td><td>{esc(s.get("rows","missing"))}</td><td>{esc(s["errors"])}</td><td><code>{esc(s.get("full_digest","missing"))}</code></td><td>{esc(disposition)}{": "+esc(s["error"]) if "error" in s else ""}</td></tr>')
for s in r['incomplete_trials']:
 page.append(f'<tr><td>{esc(s["label"])}</td><td>missing</td><td>not retained</td><td>missing</td><td>missing</td><td>missing</td><td>{esc(s["reason"])}</td></tr>')
page.append('</tbody></table></div></details><p>Method, SQL attribution, checks and limits: <a href="optimize-session-list.md">optimization report</a>. Full hashes and samples: <a href="session-list-measure/comparison-results.json">comparison receipts</a>. SQL counters: <a href="session-list-measure/comparison-profiles.json">profile receipts</a>.</p></html>')
(OUT.parent/'measure.html').write_text('\n'.join(page)+'\n')
