"""Render captured shared planning reads into the local design preview."""
import html
import json
from pathlib import Path
import markdown

ROOT = Path(__file__).resolve().parent
source = json.loads((ROOT / 'source.json').read_text())
repos = []
tasks = {}
for repo in ['loopflow', 'etude', 'kata']:
    waves = []
    for row in source['waves']:
        if Path(row['wave']['repo']).name != repo:
            continue
        raw = row['wave']
        wid = repo + '-' + raw['name']
        chapter = row.get('chapter')
        error = row.get('refresh_error') or (row['tasks'].get('reason') if row['tasks']['state'] != 'ok' else None)
        wave = {'id':wid,'name':raw['name'],'objective':raw['goal'],'color':['#783343','#597455','#567585'][len(waves)%3], 'tasks':[], 'krs':[kr['text'] for kr in chapter['krs']] if chapter else None, 'planError':error,'chapter':chapter['id'] if chapter else None}
        wave['krHtml'] = [markdown.markdown(html.escape(kr)) for kr in wave['krs']] if wave['krs'] is not None else None
        for item in row['tasks'].get('items', []):
            t = item['task']
            tid = t['identifier']
            condition = item.get('condition') or {}
            progress = condition.get('local_progress') or {}
            executed = any(any(s['selector'] == 'task:' + tid for s in run['subjects']) for run in row.get('runs',{}).get('items',[]))
            started = bool(progress.get('unsettled') or progress.get('authored_commits') or executed)
            description = t.get('description') or ''
            tasks[tid] = {'id':tid,'key':tid,'title':t['name'],'repo':repo,'wave':wid,'started':started,'state':'progress' if started else 'upcoming','stateLabel':'Completed' if t['completed'] else 'Open','brief':'','outcome':[],'sessions':[],'checkout':None,'pr':None,'directive':[description],'description':description,'directiveHtml':markdown.markdown(html.escape(description),extensions=['tables','fenced_code']),'diag':json.dumps({'condition':condition,'runtime':item.get('runtime')},ensure_ascii=False,indent=2),'issueUrl':item.get('reference',{}).get('issue_url'),'current':True,'sessionEvidence':'The current Session read has no exact Task attribution. Conversation counts are unavailable in this preview.'}
            wave['tasks'].append(tid)
        waves.append(wave)
    repos.append({'id':repo,'name':repo,'waves':waves,'loose':[]})
plan = {'observedAt':source['observed_at'],'repos':repos,'tasks':tasks,'sessionAttributionAvailable':False}
(ROOT.parent / 'current-data.js').write_text('window.CURRENT_PLAN = '+json.dumps(plan,ensure_ascii=False).replace('<','\\u003c')+';\n')
print({r['id']:sum(len(w['tasks']) for w in r['waves']) for r in repos})
