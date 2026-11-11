"""Put the final verdict above the retained historical measurement ledger."""
from html import escape
import json
import textwrap
from pathlib import Path

OUT = Path(__file__).parent
REPORT = OUT.parent
LABELS = {'cli':'Default Session list','served':'Warm served Session list','waves':'Wave list',
          'task':'Task status LOO-822, network denied','board':'Board','selection':'Work selection'}
ORDER = ('cli','served','waves','task','board','selection')


def value(side):
    low,high = side['range_ms']
    return f"{side['median_ms']:.1f} [{low:.1f}–{high:.1f}]"


def main():
    data = json.loads((OUT / 'final-results.json').read_text())
    notes = json.loads((OUT / 'final-notes.json').read_text())
    table = []
    for workload in ORDER:
        result = data['comparisons'][workload]
        before,after = [result['summary'][s] for s in ('before','after')]
        verdict = result['verdict'].capitalize()
        if result['verdict'] == 'better': verdict += f": {result['reduction_pct']:.1f}% lower"
        table.append([LABELS[workload],value(before),value(after),verdict])
    summary = ('**Better: the kept gains survive compress and sync onto main.** '
               'Fresh comparison of `00851d9c6` → `b8fd92ab6b`, five alternating pairs '
               'per workload: default Session list **24.5% lower**, served **70.7% lower**, '
               'Wave list **43.9% lower**, and network-denied Task status **85.6% lower**. '
               'Work selection is **55.9% lower**; board has **no measurable difference**. '
               'All 60 timed results and their warm-ups match complete JSON, with zero '
               'command or validation errors. **No fields differ; no main-merge exceptions are needed.**')
    historical = ('This confirms the relative gains, not the old absolute latencies: '
        'the earlier CLI 579→436 ms, served 775→211 ms, Wave 441→247 ms and Task '
        '7.3 s→1.24 s came from earlier cohorts. Here CLI is 849→640 ms, served '
        '894→262 ms, Wave 446→250 ms and Task 9.43 s→1.36 s. The fresh store and '
        'shared Machine differ; do not combine cohorts into a speedup. All five '
        'improving workloads have non-overlapping ranges. The original 18–75 s '
        'Session-list stall remains unexplained.')
    setup = ('Both exact commit exports were rebuilt in release mode with Rust 1.98.1 '
        'on macOS 26.0.1 arm64, then measured through the unchanged five-file harness. '
        'A fresh read-only online backup supplied the private snapshot: 14,804 Sessions, '
        '237,607 events, 160,530 Processes and 1,099 Tasks. Every measured lf command '
        'used /tmp/loo822-final-measure, never the real Machine store. Both migrated '
        'copies were checkpointed to DELETE mode, with no WAL; all 79 application-table '
        'digests match except schema_migrations. Baseline has none of the four optimization '
        'indexes; HEAD has exactly those four. Clock, credential-free provider copies, '
        'executable/home paths and warm-up policy match across sides. Store, harness and '
        'protected loo-822.md hashes remain unchanged.')
    validation = ('The timed region includes command execution and JSON collection; served '
        'timing covers the requested checkpoint through its answer. Answers contain 100 '
        'default Sessions and 372 served Sessions (zero raw rows), seven Waves, one Task with 90 unique '
        'Sessions, 182 board Tasks and 25 Work members. Session titles/directories and '
        'Task titles match the snapshot. One-row calibration (281.1–400.6 ms baseline, '
        '276.4–293.1 ms HEAD) is distinguishable from the calibration’s 100-row reads '
        '(573.0–584.5 and 424.0–424.5 ms). All 12 preflight answers, eight calibration '
        'answers and 14 profile answers also validate; profiles match the timed full '
        'JSON. No fields or array ordering were removed. The ten timed board calls emit '
        '40 existing unknown-crons warning lines.')
    limits = ('These are complete warm local paths, not microbenchmarks. Task status, Wave '
        'list, board and Work selection run with network denied; this does not measure '
        'successful planning exchange. Credential-free provider copies leave native '
        'Keychain fallback costs unlike some normal logins. OS processes and Git/config '
        'facts remain live. Timed one-minute Machine load was 12.3–18.6; other builds '
        'and tests were observed. The snapshot is sealed, not the Machine. No synthetic '
        'writer or load was introduced. No production source changed in this final '
        'measurement; no implementation revert is indicated. All benchmark servers '
        'and samplers exited.')
    paras = [historical,setup,validation,*notes['paragraphs'],limits]
    wrap = lambda text: textwrap.fill(text, width=88, break_long_words=False, break_on_hyphens=False)
    markdown = '# Session-list optimization measurements — LOO-822\n\n' + wrap(summary) + '\n\n'
    markdown += 'Final measurement, 2026-11-11. **Median [minimum–maximum] milliseconds; n=5 per side.**\n\n'
    markdown += '| Workload | Baseline | HEAD | Verdict |\n|---|---:|---:|---|\n'
    markdown += '\n'.join('| '+' | '.join(row)+' |' for row in table)+'\n\n'
    markdown += wrap(paras[0])+'\n\n## Final measurement details\n\n'
    markdown += '\n\n'.join(wrap(p) for p in paras[1:])+'\n\n'
    markdown += ('Receipts: [all trials and sealed-input checks](session-list-measure/final-results.json), '
                 '[excluded SQL and stack profiles](session-list-measure/final-profiles.json), '
                 '[measurement ledger](measure.html#final). Raw answers and stores remain private.\n\n')
    old = (REPORT / 'optimize-session-list.md').read_text()
    (REPORT / 'optimize-session-list.md').write_text(markdown + old[old.index('## Iteration 1 verdict'):])
    html = '<section id="final"><h2>Final confirmation after compress and sync</h2><p><strong class="keep">'+escape(summary.replace('**','').replace('`',''))+'</strong></p>'
    html += '<p>00851d9c6 → b8fd92ab6b; median [minimum–maximum] milliseconds, n=5 per side.</p><table><thead><tr><th>Workload</th><th>Baseline ms</th><th>HEAD ms</th><th>Verdict</th></tr></thead><tbody>'
    html += ''.join('<tr>'+''.join('<td>'+escape(cell)+'</td>' for cell in row)+'</tr>' for row in table)+'</tbody></table>'
    html += ''.join('<p>'+escape(p)+'</p>' for p in paras)
    html += '<details id="final-trials"><summary>Every final trial and warm-up</summary><table><thead><tr><th>Workload</th><th>Side</th><th>Timed ms</th><th>Warm-up ms</th><th>Rows</th><th>Errors</th><th>Full JSON SHA-256</th></tr></thead><tbody>'
    for row in data['trials']:
        cells = [row['workload'],row['side'],f"{row['ms']:.3f}",f"{row['warmup']['ms']:.3f}",str(row['rows']),str(row['errors']),row['full_digest']]
        html += '<tr>'+''.join('<td>'+escape(v)+'</td>' for v in cells)+'</tr>'
    html += '</tbody></table></details><p><a href="session-list-measure/final-results.json">All trials and input checks</a>; <a href="session-list-measure/final-profiles.json">SQL and stack attribution</a>.</p></section>\n'
    page = (REPORT / 'measure.html').read_text()
    if '<section id="final">' in page:
        start = page.index('<section id="final">')
        end = page.index('<section id="iteration3">', start)
        page = page[:start] + page[end:]
    page = page.replace('<section id="iteration3">',html+'<section id="iteration3">',1)
    ledger = '<table><thead><tr><th>Commit</th><th>Value</th><th>Verdict</th><th>Hypothesis</th><th>Limiter</th></tr></thead><tbody>'
    row = '<tr><td><code>b8fd92ab6b</code> vs <code>00851d9c6</code></td><td>CLI 640.4 ms; served 262.0 ms; Wave 249.9 ms; Task 1362.0 ms. n=5 per side; full ranges above.</td><td class="keep">Better: retained after compress and sync. Board: no measurable difference.</td><td>Kept query and repeated-read gains survive compress and main merge with complete JSON unchanged.</td><td>Repeated Git discovery (36.0% of HEAD Task main-thread samples), SQLite (24.6%), shared Machine load, private credential-free homes. Old absolute latencies not reproduced.</td></tr>'
    page = page.replace(row, '')
    assert ledger in page
    (REPORT / 'measure.html').write_text(page.replace(ledger,ledger+'\n'+row,1))


if __name__ == '__main__': main()
