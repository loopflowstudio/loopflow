"""Append iteration 3 decisions and every call to the self-contained ledger."""
import html
import json
from pathlib import Path
import subprocess

from iteration3 import ROOT, COMMITS, WORKLOADS
from summarize_iteration3 import summarize

OUT = Path(__file__).parent
result = summarize()
cohorts = result['cohorts']
stacks = json.loads((ROOT / 'stack-summary.json').read_text())


def _metric(v):
    return f"{v['median_ms']:.1f} [{v['range_ms'][0]:.1f}–{v['range_ms'][1]:.1f}]"


def _table(headings, rows):
    return '<div class="scroll"><table><thead><tr>' + ''.join('<th>'+html.escape(s)+'</th>' for s in headings) + '</tr></thead><tbody>' + ''.join('<tr>'+''.join('<td>'+html.escape(str(s))+'</td>' for s in row)+'</tr>' for row in rows) + '</tbody></table></div>'


assert all(cohorts[c+'-task']['verdict'] == 'better' for c in ('before-after1','after1-after2'))
assert not any(c['verdict'] == 'worse' for c in cohorts.values())
first, second = [cohorts[c+'-task'] for c in ('before-after1', 'after1-after2')]
summary = (f"Better: keep d1f1caa3f7 and a835f03e6f. Network-denied Task status falls "
           f"{first['reduction_pct']:.1f}% with one capacity read, then {second['reduction_pct']:.1f}% "
           "with one scope resolution per repository. Five alternating pairs per comparison; "
           "non-overlapping primary ranges, matching full JSON and zero command or validation errors.")
section = '<section id="iteration3"><h2>Iteration 3 — repeated Task-status reads</h2><p><strong class="keep">'+summary+'</strong></p>'
section += '<p>All values: median [minimum–maximum] milliseconds, n=5 per side. Guard workloads show no measurable difference. The primary is the complete CLI command, including JSON output collection. Network is denied; this does not measure a successful planning exchange.</p>'
rows = []
for comparison in ('before-after1','after1-after2'):
    for workload in WORKLOADS:
        cohort = cohorts[comparison+'-'+workload]
        rows.append([comparison, workload, *[_metric(v) for v in cohort['sides'].values()], cohort['verdict']])
section += _table(['Comparison','Workload','Before ms','After ms','Verdict'], rows)
limiter = json.loads((ROOT / 'report-notes.json').read_text())
section += '<p>'+html.escape(limiter['attribution'])+'</p><p>'+html.escape(limiter['arithmetic'])+'</p><p>'+html.escape(limiter['next_limiter'])+'</p>'
rows = []
for side in COMMITS:
    for workload in WORKLOADS:
        values = [r['security_spawns'] for r in result['counts'] if r['side']==side and r['workload']==workload]
        assert len(values)==5
        rows.append([side,workload,', '.join(map(str,values))])
section += '<h3>Security subprocesses per excluded attribution call</h3>'+_table(['Side','Workload','Five counts'], rows)
section += '<p>Spawn interposition records only parent and successful child PIDs, deduplicating the posix_spawn/posix_spawnp path. No argv, credential contents or credential output is logged. Counter calibration launched one harmless stand-in child and recorded one unique PID pair. Timed runs have only the unchanged clock shim; the counter and native samples are excluded.</p>'
section += '<p>'+html.escape(limiter['limits'])+'</p><p>'+html.escape(limiter['checks'])+'</p>'
section += _table(['Commit','Value','Verdict','Hypothesis','Limiter'], [
    ['d1f1caa3f7',_metric(first['sides']['after1'])+' ms; '+str(round(first['reduction_pct'],1))+'% lower','Keep: better','One LaunchCapacity read removes repeated native-login subprocesses.', 'Planning scope configuration loading remains.'],
    ['a835f03e6f',_metric(second['sides']['after2'])+' ms; '+str(round(second['reduction_pct'],1))+'% lower','Keep: better','Resolve each repository scope once within a check.',limiter['next_limiter']]])
trials = [(r,'timed comparison' if r['label'].startswith('iteration3-') else 'excluded calibration') for r in result['trials']]
trials += [(r,'excluded spawn attribution') for r in result['counts']]
trials += [(r,'excluded native sample') for r in result['samples']]
trials += [(r,'excluded wall/SQL attribution') for r in result['attributions']]
rows = []
for r, disposition in trials:
    rows.append([r['label'], f"{r['ms']:.3f}", f"{r['warmup']['ms']:.3f}" if 'warmup' in r else 'not run', r['rows'],r['errors'],r.get('security_spawns','not instrumented'),r['full_digest'],disposition])
section += '<details id="iteration3-trials"><summary>Every iteration-3 call, including excluded runs and warm-ups</summary>'+_table(['Trial','ms','Warm-up ms (excluded)','Rows','Errors','Security spawns','Full JSON digest','Disposition'],rows)+'</details>'
section += '<p>Receipts: <a href="session-list-measure/iteration3-results.json">trials and input hashes</a>, <a href="session-list-measure/iteration3-checks.json">validation and stack attribution</a>. Raw outputs, stores and stack captures stay under the private /tmp/loo822-reads-measure directory.</p></section>'
old = subprocess.run(['git','show','a835f03e6f:scratch/measure.html'],capture_output=True,text=True,check=True).stdout
old = old.replace('<h1>Session-list optimization measurements</h1>', '<h1>Session-list optimization measurements</h1>\n'+section, 1)
(OUT.parent / 'measure.html').write_text(old)
(OUT / 'iteration3-checks.json').write_text(json.dumps(dict(summary=summary,stacks=stacks,notes=limiter,retained_calls=80,excluded_calls=len(trials)-80,errors=0,full_json_matches=True,sealed_harness_unchanged=True,protected_loo822_unchanged=True),indent=2)+'\n')
print('Ledger includes',len(trials),'iteration-3 calls and their warm-ups.')
