const { chromium } = require('/Users/jack/.npm/_npx/a5b920f00216d246/node_modules/playwright');
const assert = require('node:assert/strict');
(async () => {
 const browser = await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});
 const page = await browser.newPage({viewport:{width:1320,height:980},deviceScaleFactor:1});
 const errors = []; page.on('pageerror',e=>errors.push(e.message));
 await page.goto('file:///Users/jack/src/loopflow.main-view-task/scratch/hierarchy-study.html');
 const draft=page.locator('textarea[data-session="navigation-session"]');
 await draft.fill('Keep this unfinished thought');
 for (const mode of ['compact','full','sessions']) {
   await page.selectOption('#presentation',mode);
   assert.equal(await draft.inputValue(),'Keep this unfinished thought');
   assert.equal(await draft.isVisible(),true);
   assert.equal(await page.locator('.row.selected').getAttribute('data-id'),'navigation-session');
   await page.screenshot({path:`/tmp/loo291-review17/${mode}.png`});
 }
 assert.equal(await page.locator('#outline .row').count(),6);
 assert.equal(await page.locator('#outline .context').count(),6);
 await page.locator('[data-id="recovery-session"] .item').click();
 await page.locator('textarea[data-session="recovery-session"]').fill('Independent draft');
 await page.locator('[data-id="navigation-session"] .item').click();
 assert.equal(await draft.inputValue(),'Keep this unfinished thought');
 await page.selectOption('#presentation','compact');
 await page.getByRole('button',{name:'Desktop Project details',exact:true}).click();
 assert.equal(await page.locator('#details h1').textContent(),'Desktop');
 await page.getByRole('button',{name:'Collapse Product',exact:true}).click();
 assert.equal(await page.locator('[data-id="navigation-session"]').count(),0);
 await page.getByRole('button',{name:'Expand Product',exact:true}).click();
 await page.locator('[data-id="startup"] .item').click();
 assert.match(await page.locator('#details').textContent(),/Upcoming · no open Session/);
 await page.locator('[data-id="unknown-session"] .item').click();
 assert.equal(await page.locator('textarea[data-session="unknown-session"]').isVisible(),true);
 assert.deepEqual(errors,[]);
 console.log('PASS: six Sessions reachable; stable selection/drafts across three presentations; separate same-name drafts; compressed Project details; folding; upcoming Task; unknown ancestry. Mock interaction only.');
 await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});
