import json
from pathlib import Path
from playwright.sync_api import sync_playwright

URL = 'http://127.0.0.1:8317/scratch/visual-study/mockups.html?v=a'
DRAFT = 'Keep this exact thought — Task to Session ✓'
results = []
with sync_playwright() as p:
    browser = p.chromium.launch(executable_path=str(next(Path('/Users/jack/Library/Caches/ms-playwright/chromium_headless_shell-1228').rglob('chrome-headless-shell'))), headless=True)
    for width, height in [(1400, 780), (1100, 720)]:
        page = browser.new_page(viewport={'width': width, 'height': height})
        errors = []
        page.on('pageerror', lambda e: errors.append(str(e)))
        page.goto(URL)
        page.set_default_timeout(5000)
        def zoom():
            return page.locator('.zoom [aria-pressed=true]').get_attribute('data-z')
        def task(task_id):
            page.locator(f'.outline [data-act=opentask][data-id={task_id}]').click()
        def overview():
            page.locator('.zoom [data-z="0"]').click()
        assert zoom() == '0'
        assert page.locator('.outline .sess-row [data-act=selsession]').count() == 0
        assert page.locator('.outline [data-act=task]').count() == 0
        page.locator('[data-draft="s-desktop"]').fill(DRAFT)
        page.locator('.wbar [data-act=togglemon]').click()
        task('loo291')
        assert zoom() == '1'
        assert page.locator('[data-draft="s-desktop"]').input_value() == DRAFT
        assert page.locator('.pane').count() == 1
        overview()
        assert page.locator('.pane').count() == 3
        task('loo295')
        assert zoom() == '0'
        assert page.locator('[data-act=startconv]').count() > 0
        assert page.locator('[data-draft]').count() == 0
        task('loo293')
        assert zoom() == '0'
        assert page.locator('[data-act=movehere]').count() > 0
        assert page.locator('[data-draft="s-watch"]').count() == 0
        task('loo291')
        overview()
        page.locator('.outline [data-act=inspecttask][data-id=loo291]').click()
        assert zoom() == '0'
        assert page.locator('.outline [data-act=inspecttask][data-id=loo291]').inner_text().strip() == '1'
        assert page.locator('.outline [data-act=inspecttask][data-id=loo293]').inner_text().strip() == '1'
        assert page.locator('.outline [data-act=inspecttask][data-id=loo295]').count() == 0
        page.locator('[data-act=newconv][data-id=loo291]').click()
        overview()
        task('loo295')
        task('loo291')
        assert zoom() == '0'
        assert page.locator('.outline [data-act=inspecttask][data-id=loo291]').inner_text().strip() == '2'
        assert page.locator('.sessions-list [data-act=selsession]').count() == 2
        page.locator('.sessions-list [data-act=selsession][data-id=s-desktop]').click()
        assert zoom() == '1'
        assert page.locator('[data-draft="s-desktop"]').input_value() == DRAFT
        overview()
        assert page.locator('.pane').count() == 3
        page.locator('.hrepo [data-act=repomenu]').click()
        page.locator('[data-act=scope][data-id=cube]').click()
        assert page.locator('[data-inp=q]').get_attribute('placeholder') == 'Find in cube'
        page.locator('.hrepo [data-act=repomenu]').click()
        page.locator('[data-act=scope][data-id=loopflow]').click()
        assert page.locator('[data-draft="s-desktop"]').input_value() == DRAFT
        assert page.locator('.outline [data-act=selloose]').count() == 1
        assert not errors, errors
        results.append({'viewport': [width, height], 'single_session_direct_entry': True, 'zero_and_external_explicit': True, 'multiple_exact_picker': True, 'draft_companion_monitor_retained': True, 'repo_round_trip': True, 'no_nested_session_rows': True, 'errors': errors})
        page.close()
    browser.close()
Path('scratch/visual-study/task-entry-evidence/interactions.json').write_text(json.dumps(results, indent=2) + '\n')
print(json.dumps(results, indent=2))
