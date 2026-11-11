"""Append iteration 2 to the self-contained ledger without losing earlier trials."""
import hashlib
import html
import json
from pathlib import Path
import re
import subprocess

ROOT = Path('/tmp/loo822-process-measure')
OUT = Path(__file__).parent
result = json.loads((OUT / 'iteration2-results.json').read_text())
profiles = json.loads((OUT / 'iteration2-profiles.json').read_text())['reads']
stacks = json.loads((ROOT / 'stack-summary.json').read_text())
cohorts = result['cohorts']
for p in profiles:
    expected = cohorts[p['workload']]['sides'][p['side']]['full_digest']
    assert p['full_digest'] == expected and p['warmup']['full_digest'] == expected
for side, stack in stacks.items():
    assert stack['receipt']['full_digest'] == cohorts['task']['sides'][side]['full_digest']
for name, expected in result['manifest']['harness_sha256'].items():
    assert hashlib.sha256((OUT / name).read_bytes()).hexdigest() == expected


def metric(v):
    return f"{v['median_ms']:.1f} [{v['range_ms'][0]:.1f}–{v['range_ms'][1]:.1f}]"


verdict = 'Better: keep 761c28056.'
summary = ('Wave list: 441.4 → 246.5 ms median (44.2% lower). Task status, network denied: '
           '7373.0 → 6864.7 ms (6.9% lower). Five alternating pairs per workload, '
           'non-overlapping ranges, matching full JSON, zero command/validation errors. '
           'Session lists and Work selection show no measurable difference. Board is inconclusive.')
limiter = ('Task status still spends most sampled main-thread time checking Claude logins through '
           'security (59–62%); planning configuration loading accounts for 16–17%. '
           'Private provider homes, ambient OS/Git state and shared-Machine load limit generalization.')
checks = dict(verdict=verdict, summary=summary, limiter=limiter, stacks=stacks,
              full_json_matches_profiles_and_samples=True, timing_harness_unchanged=True,
              retained_timed_calls=60, retained_warmups=60, command_or_validation_errors=0,
              preparation_failures=1, excluded_timed_calls=60,
              board_attribution='Neither new index is used. Substantive bound plans and work counters match; the small timing separation is unexplained.',
              private_directory=str(ROOT), loo_822_untouched=True)
existing_checks = json.loads((OUT / 'iteration2-checks.json').read_text()) if (OUT / 'iteration2-checks.json').exists() else {}
existing_checks.update(checks)
(OUT / 'iteration2-checks.json').write_text(json.dumps(existing_checks, indent=2)+'\n')

header = f'<section id="iteration2"><h2>Iteration 2 — sealed private stores</h2><p><strong class="keep">{verdict}</strong> {summary}</p><p>{limiter}</p>'
header += '<p>Before: <code>2667788c6</code>. After: <code>761c28056</code>. All times are milliseconds, median [minimum–maximum], n=5 per side. The first cohort is invalid because its baseline copy omitted a WAL sidecar; its apparent Work-selection gain is withdrawn. Both databases were then checkpointed, sealed in DELETE mode, and read with <code>immutable=1</code> to verify standalone indexes and all 78 application-table digests. The unchanged timing harness was rerun in full.</p>'
header += '<table><thead><tr><th>Workload</th><th>Before ms</th><th>After ms</th><th>Verdict</th><th>Rows</th></tr></thead><tbody>'
for name, group in cohorts.items():
    b, a = group['sides']['before'], group['sides']['after']
    header += f'<tr><td>{name}</td><td>{metric(b)}</td><td>{metric(a)}</td><td>{group["verdict"]}</td><td>{a["rows"]}</td></tr>'
header += '</tbody></table><p>Board’s ranges barely separate, but neither new index appears in its bound plans and its substantive SQL work is unchanged. No causal speedup is claimed.</p>'
header += '<table><thead><tr><th>Target statement</th><th>Before fullscan / VM</th><th>After fullscan / VM</th><th>Source</th></tr></thead><tbody><tr><td>Unfinished agents</td><td>160,313 / 674,239</td><td>58 / 2,133</td><td>store/sqlite/processes.rs::agent_processes</td></tr><tr><td>Task Process membership</td><td>175,320 / 7,641,629</td><td>15,007 / 274,770</td><td>store/sqlite/task_work.rs::lf_process_ids</td></tr></tbody></table>'
header += '<p>Bundled SQLite 3.53.2; separate release attribution builds. Wave inventory: 187.6 → 0.3 ms SQL in a 461.5 → 303.7 ms traced path. The SQL is 40.7% of that baseline, so its 34.2% traced path reduction fits the bound. Task inventory plus membership: 567.6 → 6.3 ms, 7.1% of the 7960.3 ms traced baseline. The uninstrumented reductions (194.9 and 508.3 ms) are consistent with hundreds of milliseconds of removed SQL, within different runs’ noise; instrumentation changes total time, so profile shares are not exact predictions for the timing cohort. Nested statement times are not summed.</p>'
header += '<p>Warm native samples: before 752/1267 (59.4%) and after 686/1105 (62.1%) main-thread samples in Claude Keychain reads; configuration loading 213/1267 (16.8%) and 182/1105 (16.5%). Stack: task_detail → human_session::listed → LaunchCapacity::read → same_login → read_claude_keychain_blob_with_security → child output/poll. The Task returns 84 Session rows, and listed rereads launch capacity per row. Planning refresh separately resolves configuration inside its Task loop. These samples are excluded from timing. Private transcript copies omit credentials, so native-login fallback costs may differ from ordinary homes.</p>'
header += '<p>Fresh frozen store: 14,759 Sessions, 236,791 events, 12,983 activity rows, 160,314 Processes, 1,099 Tasks and six accounts. Raw answers and snapshots remain private. The 60 timed calls have zero failures; 40 existing unknown-crons warning lines occur across the ten board reads. Network rejection is intentional and not successful planning exchange. All timing/profile answers match without removing timestamps, paths or fields.</p></section>'

old = subprocess.run(['git', 'show', '761c28056:scratch/measure.html'], capture_output=True, text=True, check=True).stdout
old = old.replace('<h1>Session-list optimization measurements</h1>', '<h1>Session-list optimization measurements</h1>\n'+header+'\n<h2>Earlier iterations</h2>', 1)
row = f'<tr><td><code>761c28056</code></td><td>Wave {metric(cohorts["waves"]["sides"]["after"])} vs {metric(cohorts["waves"]["sides"]["before"])} ms; Task {metric(cohorts["task"]["sides"]["after"])} vs {metric(cohorts["task"]["sides"]["before"])} ms; n=5.</td><td class="keep">Kept: better.</td><td>Indexed live-agent and Task Process inventories reduce local planning latency without changing answers.</td><td>{limiter} First unsealed cohort rejected.</td></tr>'
# The original decision table follows the new section's two tables.
position = old.index('</tbody></table>', old.index('<h2>Earlier iterations</h2>'))
old = old[:position] + row + old[position:]

trials = [(r, 'retained comparison' if r['label'].startswith('iteration2-') else 'excluded: calibration') for r in result['trials']]
trials += [(r, 'INVALID first cohort: baseline WAL omitted') for r in result['excluded_cohort']['trials']]
trials += [(r, 'excluded: SQL attribution') for r in profiles]
trials += [(r, 'INVALID cohort attribution') for r in json.loads((ROOT/'unsealed/profiles.json').read_text())]
for directory, disposition in ((ROOT, 'excluded: native sample'), (ROOT/'unsealed', 'excluded: initial native sample')):
    for side in ('before', 'after'):
        r = json.loads((directory/f'sample-{side}-receipt.json').read_text())
        r['label'] = 'sample-' + side
        trials.append((r, disposition))
exploratory = json.loads((ROOT/'exploratory-before-receipt.json').read_text())
exploratory['label'] = 'exploratory-before'
trials.append((exploratory, 'excluded: exploratory sample during compilation'))
trials += [(r, 'excluded: migration preparation') for r in json.loads((ROOT/'unsealed/preparations.json').read_text())]
trials.append((json.loads((ROOT/'unsealed/selection-check.json').read_text()), 'excluded: diagnosed omitted indexes'))
trials.append((dict(label='early-private-migration-preflight', ms=7636.007625143975, rows=1, errors=0, full_digest='8b6742c4051dbab7f83a4b136913c410dc6d0907ebc017909f70bbdddcd1859e'), 'excluded: initial migration preflight'))
ledger = '<details id="iteration2-trials"><summary>Every iteration-2 trial, including invalid and profiling runs</summary><table><thead><tr><th>Trial</th><th>ms</th><th>Warm-up ms</th><th>Rows</th><th>Errors</th><th>Full JSON digest</th><th>Disposition</th></tr></thead><tbody>'
for r, disposition in trials:
    warm = f'{r["warmup"]["ms"]:.3f}' if 'warmup' in r else 'not run'
    ledger += f'<tr><td>{html.escape(r["label"])}</td><td>{r["ms"]:.3f}</td><td>{warm}</td><td>{r["rows"]}</td><td>{r["errors"]}</td><td><code>{r["full_digest"]}</code></td><td>{disposition}</td></tr>\n'
ledger += '</tbody></table></details><p>Iteration-2 receipts: <a href="session-list-measure/iteration2-results.json">all comparisons</a>, <a href="session-list-measure/iteration2-profiles.json">SQL plans and counters</a>, <a href="session-list-measure/iteration2-checks.json">validation and stack attribution</a>.</p>'
old = old.replace('</html>', ledger+'</html>')
(OUT.parent/'measure.html').write_text(old)
print(f'Wrote ledger with {len(trials)} iteration-2 trial rows plus earlier iterations.')
