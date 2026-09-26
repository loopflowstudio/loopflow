import json
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent
source = json.loads((ROOT / 'source.json').read_text())
with sync_playwright() as p:
    browser = p.chromium.launch(executable_path=str(next(Path('/Users/jack/Library/Caches/ms-playwright/chromium_headless_shell-1228').rglob('chrome-headless-shell'))), headless=True)
    page = browser.new_page(viewport={'width': 1400, 'height': 900})
    page.set_default_timeout(5000)
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.goto('http://127.0.0.1:8317/scratch/visual-study/mockups.html?v=a&population=current&task=LOO-291')
    page.wait_for_selector('.current-task')
    actual = page.evaluate('({repos:REPOS.map(r=>r.id),tasks:Object.values(TASKS).map(t=>({id:t.id,title:t.title,description:t.description})),waves:REPOS.flatMap(r=>r.waves).map(w=>({id:w.id,objective:w.objective,krs:w.krs}))})')
    assert actual['repos'] == ['loopflow', 'etude', 'kata']
    for row in source['waves']:
        wid = Path(row['wave']['repo']).name + '-' + row['wave']['name']
        wave = next(w for w in actual['waves'] if w['id'] == wid)
        assert wave['objective'] == row['wave']['goal']
        assert wave['krs'] == ([k['text'] for k in row['chapter']['krs']] if row.get('chapter') else None)
        for item in row['tasks'].get('items', []):
            task = item['task']
            rendered = next(t for t in actual['tasks'] if t['id'] == task['identifier'])
            assert rendered['title'] == task['name']
            assert rendered['description'] == (task.get('description') or '')
    assert len(actual['tasks']) == 12
    assert page.locator('.task-heading h1').inner_text() == next(t['title'] for t in actual['tasks'] if t['id'] == 'LOO-291')
    assert page.locator('[data-act=newconv],[data-act=startconv],[data-act=movehere],.term').count() == 0
    for repo in ['etude', 'kata', 'loopflow']:
        page.locator('.hrepo [data-act=repomenu]').click()
        page.locator(f'[data-act=scope][data-id={repo}]').click()
        assert page.locator('[data-inp=q]').get_attribute('placeholder') == 'Find in ' + repo
        if repo == 'kata':
            assert page.locator('.plan-gap').is_visible()
            assert 'No active Tasks' not in page.locator('.wavepage').inner_text()
        if repo == 'etude':
            assert page.locator('.wavepage h1').inner_text() == 'game'
            page.locator('.wavepage [data-act=opentask][data-id=ETU-77]').click()
            assert 'Make large legal decisions navigable' in page.locator('.task-heading h1').inner_text()
            assert page.locator('.outline [data-id=ETU-77]').count() == 0
    assert not errors, errors
    browser.close()
(ROOT / 'checks.json').write_text(json.dumps({'exact_task_names_and_descriptions': 12, 'exact_wave_objectives_and_krs': 14, 'repos': actual['repos'], 'repo_switching': True, 'kata_unavailable_not_empty': True, 'no_fake_sessions_or_mutation_controls': True, 'errors': errors}, indent=2) + '\n')
print('Exact source comparison and three-repository interactions pass.')
