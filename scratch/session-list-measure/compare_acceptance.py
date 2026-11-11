"""Untimed full-answer comparisons for filters and planning read scopes."""
import json
import sqlite3
import compare as c
from planning_compare import cli

r=c.ROOT
session=json.loads((r/'frozen-preflight-cli.json').read_text())[0]['id']
waves=json.loads((r/'preflight-waves.json').read_text())
wave=waves[0]['name']
cases={
 'task-filter':['session','list','--task','LOO-822','--json'],
 'orphan':['session','list','--orphan','--json'],
 'history':['session','list','--history','--json'],
 'page':['session','list','--page','--limit','3','--json'],
 'card':['session','card',session,'--json'],
 'board-all':['plan','board','--all','--json'],
 'wave-status':['wave','status',wave,'--json'],
 'wave-status-active':['wave','status',next(w['name'] for w in waves if w['active_tasks']>0),'--json'],
}
results=[]
for case,args in cases.items():
 expected=None
 for side in ('before','after1','after2'):
  c.activate(side);c.COMMANDS['acceptance']=args
  ms,data,stderr=cli('acceptance',c.environment())
  if case in ('task-filter','orphan','history'): checked=c.check(data,c.HOME/'loopflow.db')
  elif case=='page':checked=c.check(data['entries'],c.HOME/'loopflow.db')
  elif case=='card':checked=c.check([data['session']],c.HOME/'loopflow.db')
  else:checked={'rows':len(data['tasks']['items'])}
  digest=c.digest(data)
  if expected is None:expected=digest
  assert digest==expected,f'{case} differs on {side}'
  if case=='orphan':assert all(not row['task_ids'] for row in data)
  if case=='page':
   after=data['next']
   c.COMMANDS['acceptance']=[*args,'--after',after]
   _,second,_=cli('acceptance',c.environment())
   assert not set(x['id'] for x in data['entries']) & set(x['id'] for x in second['entries'])
   if side=='before':second_expected=c.digest(second)
   assert c.digest(second)==second_expected
  row=dict(case=case,side=side,full_digest=digest,rows=checked['rows'],errors=0)
  results.append(row);print(json.dumps(row),flush=True)
  (r/'acceptance.json').write_text(json.dumps(results,indent=2))
