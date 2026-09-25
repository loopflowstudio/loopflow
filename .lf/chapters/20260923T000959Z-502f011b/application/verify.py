"""Verify accepted chapter application against retained owner receipts."""

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / 'application'


def read(name: str) -> object:
    return json.loads((ROOT / name).read_text())


def verify() -> dict[str, bool]:
    proposal = read('proposal.json')
    checks = {}
    snapshots = {w: read(f'application/{w}-pm-final.stdout') for w in ['infrastructure', 'intelligence', 'product', 'list']}
    projects = {p['id']: p for s in snapshots.values() for p in s['projects']}
    items = {i['id']: i for s in snapshots.values() for i in s['items']}
    checks['five_projects_exact_ids'] = set(projects) == {p['project_id'] for p in proposal['projects']}
    checks['definitions_and_seven_unproven_krs_exact'] = all(
        projects[p['project_id']]['name'] == p['name']
        and projects[p['project_id']]['definition'] == p['definition']
        and projects[p['project_id']]['krs'] == [{'text': kr['text'], 'holds': False} for kr in p['krs']]
        for p in proposal['projects']
    ) and sum(len(p['krs']) for p in projects.values()) == 7
    expected = read('application/expected-tasks.json')
    checks['twelve_existing_tasks_exact'] = len(expected) == 12 and all(
        all(items[t['id']][key] == t[key] for key in ['name', 'description', 'project_id', 'completed'])
        for t in expected
    )
    new = read('application/new-task-plan.json')
    checks['seven_new_tasks_exact'] = len(new) == 7 and all(
        items[t['id']]['name'] == t['title']
        and items[t['id']]['description'] == t['description']
        and items[t['id']]['project'] == t['project']
        and not items[t['id']]['completed']
        for t in new
    )
    checks['eighteen_open_tasks_no_extra'] = {t['id'] for t in items.values() if not t['completed']} == (
        {t['id'] for t in expected if not t['completed']} | {t['id'] for t in new}
    )
    old = read('frozen-ledger.json')
    starting = {p['id']: p for w in old['waves'] for p in w['projects']}
    dispositions = read('project-dispositions.json')
    checks['all_nine_project_dispositions'] = len(dispositions) == 9 and {p['project_id'] for p in dispositions} == set(starting)
    krs = read('prior-kr-dispositions.json')
    claims = {(p['id'], i + 1, kr['text']) for p in starting.values() for i, kr in enumerate(p['krs'])}
    checks['42_prior_claims_unchanged_and_dispositioned'] = len(krs) == 42 and {(r['project_id'], r['ordinal'], r['exact_claim']) for r in krs} == claims and all(r['disposition'] for r in krs)
    history = read('historical-work-dispositions.json')
    checks['174_work_dispositions_verified'] = len(history) == 174 and all(r.get('application_status') in ['retired_verified', 'carried_verified'] for r in history)
    checks['119_stale_tasks_seven_projects_retired'] = sum(r.get('application_status') == 'retired_verified' and r['kind'] == 'task' for r in history) == 119 and sum(r.get('application_status') == 'retired_verified' and r['kind'] != 'task' for r in history) == 7
    checks['terminal_history_and_active_tasks_carried'] = sum(r.get('application_status') == 'carried_verified' and r['kind'] == 'task' for r in history) == 43
    checks['no_pre_application_historical_mismatch'] = not read('application/historical-reconciliation.json')['mismatches']
    checks['retired_waves_abandoned_disabled'] = all(
        read(f'application/wave-after-{w}.stdout')['status'] == 'abandoned'
        and not read(f'application/wave-after-{w}.stdout')['placement']['enabled']
        for w in ['list', 'engbot']
    )
    waves = {w['name']: w for w in read('application/waves-final.stdout')}
    checks['final_wave_states_and_identity'] = all(
        waves[r['name']]['id'] == r['wave_id']
        and waves[r['name']]['status'] == ('abandoned' if r['disposition'] == 'retire' else 'ready')
        for r in read('wave-dispositions.json')
    )
    current_tasks = {}
    for wave in ['infrastructure', 'intelligence', 'product', 'list']:
        status = read(f'application/{wave}-status-final.stdout')
        for project in status['projects']:
            for task in project['tasks']:
                if task.get('runtime'):
                    current_tasks[task['runtime']['work_id']] = task
    active = [r for r in history if r['kind'] == 'task' and not r['pm_completed']]
    checks['both_active_task_artifacts_unchanged'] = len(active) == 2 and all(
        current_tasks[r['runtime']['work_id']]['reference'] == r['reference']
        and current_tasks[r['runtime']['work_id']]['prs'] == r['prs']
        and current_tasks[r['runtime']['work_id']]['runtime']['status'] == 'ready'
        for r in active
    )
    repo = ROOT.parents[2]
    checks['three_active_charters'] = sorted(p.parent.name for p in (repo / 'wave').glob('*/GOAL.md')) == ['infrastructure', 'intelligence', 'product']
    checks['obsolete_metric_preserved_outside_active_contracts'] = not (repo / 'wave/product/metrics/task-loop-trust.md').exists() and (APP / 'retired-contracts/task-loop-trust.md').exists()
    metadata = read('metadata.json')
    checks['frozen_sources_unchanged'] = all(hashlib.sha256((repo / x['archive']).read_bytes()).hexdigest() == x['sha256'] for x in metadata['sources'])
    receipts = [json.loads(line) for line in (APP / 'operations.jsonl').read_text().splitlines()]
    checks['every_owner_command_succeeded'] = all(r['exit_code'] == 0 for r in receipts)
    checks['no_publication_or_execution_launch'] = all(
        not any(verb in r['command'] for verb in ['publish', 'submit', 'arm', 'land', '--push'])
        and r['command'][-1:] != ['-p']
        and not (r['command'][0] == 'lf' and r['command'][1:3] in [['task', 'run'], ['project', 'run']])
        for r in receipts
    )
    checks['desktop_order_and_discord_parked'] = {r['identifier']: r['frontier'] for r in read('task-dispositions.json')}.items() >= {'LOO-251':'opening', 'LOO-280':'conditional_same_slot', 'LOO-185':'parked'}.items()
    return checks


if __name__ == '__main__':
    results = verify()
    print(json.dumps(results, indent=2))
    if not all(results.values()):
        raise SystemExit('Chapter verification failed; do not seal.')
