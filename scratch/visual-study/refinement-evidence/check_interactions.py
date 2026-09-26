import json
from pathlib import Path
from playwright.sync_api import sync_playwright

url='http://127.0.0.1:8317/scratch/visual-study/mockups.html?v=a'
exact='A quieter workspace — keep this unfinished thought ✓'
rows=[]
with sync_playwright() as p:
    browser=p.chromium.launch(executable_path=str(next(Path('/Users/jack/Library/Caches/ms-playwright/chromium_headless_shell-1228').rglob('chrome-headless-shell'))),headless=True)
    for width,height in [(1400,780),(1100,720)]:
        page=browser.new_page(viewport={'width':width,'height':height})
        page.set_default_timeout(5000)
        errors=[]
        page.on('pageerror',lambda error:errors.append(str(error)))
        page.goto(url)
        page.locator('[data-draft="s-desktop"]').fill(exact)
        page.locator('.wbar [data-act=togglemon]').click()
        assert page.locator('.pane').count()==3
        for level in [1,2]:
            page.locator(f'.zoom [data-z="{level}"]').click()
            assert page.locator('.pane').count()==1
            assert page.locator('[data-draft="s-desktop"]').input_value()==exact
            assert page.locator('.zoom [aria-pressed=true]').get_attribute('data-z')==str(level)
        page.keyboard.press('Escape')
        assert page.locator('.zoom [aria-pressed=true]').get_attribute('data-z')=='1'
        page.keyboard.press('Escape')
        assert page.locator('.pane').count()==3
        assert page.locator('[data-draft="s-desktop"]').input_value()==exact
        assert page.locator('.loc.elsewhere,.loc.here').count()==0
        page.locator('[data-act=seltask][data-id=loo295]').first.click()
        assert page.locator('.zoom [data-z="1"]').get_attribute('aria-disabled')=='true'
        page.locator('[data-act=seltask][data-id=loo293]').first.click()
        assert page.locator('.zoom [data-z="1"]').get_attribute('aria-disabled')=='true'
        assert page.locator('[data-act=movehere]').count()>0
        page.locator('[data-act=seltask][data-id=loo291]').first.click()
        assert page.locator('[data-draft="s-desktop"]').input_value()==exact
        assert page.locator('.repo-row').count()==0
        assert page.locator('[data-act=seltask][data-id=cub12]').count()==0
        search=page.locator('[data-inp=q]')
        assert search.get_attribute('placeholder')=='Find in loopflow'
        assert search.bounding_box()['y'] > page.locator('.side .scroll').bounding_box()['y'] + page.locator('.side .scroll').bounding_box()['height'] - 1
        search.fill('export')
        assert page.locator('.nomatch').count()==1
        search.fill('')
        def scope(name):
            page.locator('.hrepo [data-act=repomenu]').click()
            page.locator(f'[data-act=scope][data-id={name}]').click()
        scope('cube')
        assert page.locator('[data-inp=q]').get_attribute('placeholder')=='Find in cube'
        assert page.locator('[data-act=seltask][data-id=cub12]').count()==1
        assert page.locator('[data-act=seltask][data-id=loo291]').count()==0
        assert page.locator('.err-row').is_visible()
        scope('loopflow')
        assert page.locator('[data-draft="s-desktop"]').input_value()==exact
        assert page.locator('.pane').count()==3
        page.locator('.zoom [data-z="2"]').click()
        page.keyboard.press('/')
        assert page.locator('[data-inp=q]').evaluate('(el)=>el===document.activeElement')
        assert page.locator('.zoom [aria-pressed=true]').get_attribute('data-z')=='0'
        scope('all')
        assert page.locator('.repo-row').count()==2
        scope('loopflow')
        assert not errors,errors
        rows.append({'viewport':[width,height],'exact_draft_through_zoom':True,'companion_monitor_restore':True,'no_implicit_launch_or_transfer':True,'location_badges_removed':True,'repo_scope_and_return':True,'search_at_bottom_and_scoped':True,'slash_restores_search':True,'js_errors':errors})
        page.close()
    browser.close()
Path('scratch/visual-study/refinement-evidence/interactions.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps(rows,indent=2))
