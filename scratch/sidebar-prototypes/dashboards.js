/* Fictional interaction model; no Loopflow API, native client or persistent store. */
class DashboardStudy {
  constructor() {
    this.work = new Map([
      ['repo', {kind:'Repository', title:'loopflow', note:'What would you like to work on?'}],
      ['product', {kind:'Wave', title:'Product', note:'Keep the work understandable as it moves.'}],
      ['task-design', {kind:'Task', title:'Dashboard exploration', wave:'product', state:'started', needsYou:true, note:'The repo dashboard keeps its composition while a Task takes the stage.'}],
      ['task-theme', {kind:'Task', title:'One coherent appearance', wave:'product', state:'unstarted', note:'Filed, with no conversation opened. This study uses a local draft placeholder.'}],
      ['task-input', {kind:'Task', title:'Retain terminal input', wave:'product', state:'paused', note:'Paused work remains unfinished.'}],
    ].map(([key, value]) => [key, {...value, draft:'', fileDraft:'# Design notes\n', content:'conversation', split:false}]));
    this.repo = ['repo'];
    this.destination = {kind:'repo'};
    this.selected = 'repo';
    this.back = [];
    this.repoFocus = 'repo';
    this.waveFocus = new Map();
    this.collapsed = new Set();
    this.expandable = false;
    this.unavailable = false;
    this.lastWave = new Map();
  }
  waveKeys(wave) {
    if (this.unavailable) return this.lastWave.get(wave) ?? [wave];
    const keys = [wave, ...[...this.work].filter(([,w]) => w.wave === wave && !['completed','canceled','duplicate'].includes(w.state)).map(([key]) => key)];
    this.lastWave.set(wave, keys);
    return keys;
  }
  visible() {
    if (this.destination.kind === 'repo') return [...this.repo];
    if (this.destination.kind === 'wave') return [...this.waveKeys(this.destination.key)];
    return [this.destination.key];
  }
  visit(destination, selected) {
    this.back.push({destination:this.destination, selected:this.selected});
    this.destination = destination;
    this.focus(selected);
  }
  focus(key) {
    if (!this.visible().includes(key)) key = this.visible()[0] ?? null;
    this.selected = key;
    if (this.destination.kind === 'repo') this.repoFocus = key;
    if (this.destination.kind === 'wave') this.waveFocus.set(this.destination.key, key);
  }
  home() { this.visit({kind:'repo'}, this.repoFocus); }
  single(key) { this.visit({kind:'single', key}, key); }
  conversation(key) { this.work.get(key).content = 'conversation'; this.single(key); }
  wave(key) { this.visit({kind:'wave', key}, this.waveFocus.get(key) ?? key); }
  previous() {
    const previous = this.back.pop();
    if (previous) { this.destination = previous.destination; this.focus(previous.selected); }
  }
  add(key) {
    if (!this.work.has(key)) return;
    if (!this.repo.includes(key)) this.repo.push(key);
    if (this.destination.kind === 'repo') this.focus(key);
  }
  remove(key) {
    if (this.destination.kind !== 'repo' || key === 'repo') return;
    const index = this.repo.indexOf(key);
    this.repo = this.repo.filter(k => k !== key);
    if (this.selected === key) this.focus(this.repo[Math.max(0,index-1)]);
  }
  move(key, delta) {
    if (this.destination.kind !== 'repo') return;
    const index = this.repo.indexOf(key), next = index + delta;
    if (index < 0 || next < 0 || next >= this.repo.length) return;
    [this.repo[index], this.repo[next]] = [this.repo[next], this.repo[index]];
  }
  update(key, state) {
    this.work.get(key).state = state;
    this.focus(this.selected);
  }
}
if (typeof module !== 'undefined') module.exports = {DashboardStudy};
if (typeof document !== 'undefined') {
  const study = new DashboardStudy();
  const element = id => document.getElementById(id);
  const escape = text => text.replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
  const button = (action, key, label, extra='') => `<button data-action="${action}" data-key="${key}" ${extra}>${label}</button>`;
  const pane = (key, work) => {
    const files = work.content === 'files';
    return `<div class="pane ${files?'file':''}"><div class="pane-title"><span>${files?'Notes.md · unsaved':'Conversation · '+escape(work.title)}</span>${button('content', key, files?'Conversation':'Notes')}${button('split',key,work.split?'Close shell':'Split shell')}</div><div class="transcript">${files?'':`<p class="prompt">${escape(work.title)}</p><p>${escape(work.note)}</p>`}<textarea class="draft" data-key="${key}" data-field="${files?'fileDraft':'draft'}" aria-label="${escape(work.title)} ${files?'file':'conversation'} draft" placeholder="Leave a draft, then switch Work…">${escape(files?work.fileDraft:work.draft)}</textarea></div></div>${work.split?'<div class="pane"><div class="pane-title">Shell · retained inside this Work</div><div class="transcript">❯ <span class="prompt">No command runs in this study.</span></div></div>':''}`;
  };
  function render() {
    const kind = study.destination.kind, wave = study.destination.key ?? 'product';
    const sessionOnly = kind === 'repo' && study.repo.length === 1;
    element('title').textContent = kind === 'repo' ? (sessionOnly ? 'loopflow' : 'Your dashboard') : study.work.get(wave).title;
    element('scope').textContent = kind === 'repo' ? 'loopflow / pinned' : kind === 'wave' ? 'Wave / fixed query' : 'Work / workspace';
    element('notice').textContent = study.unavailable ? 'Planning unavailable. Last known Wave membership is retained.' : kind === 'wave' ? 'Wave conversation + unfinished Tasks, including paused and unstarted. Draft query choice.' : kind === 'repo' ? 'Add Work, arrange it, and return here whenever you need the whole picture.' : 'This same workspace appears in dashboards. Its panes and drafts stay with it.';
    element('scope').hidden = sessionOnly;
    element('notice').hidden = sessionOnly;
    element('actions').innerHTML = button('back','','Back',study.back.length?'':'disabled') + (kind === 'repo' ? `<select id="add-work" aria-label="Add Work"><option value="">Add Work…</option>${[...study.work].filter(([key])=>key!=='repo').map(([key,w])=>`<option value="${key}">${escape(w.kind+' · '+w.title)}</option>`).join('')}</select>` : button('repo','','Repo dashboard')) + (kind === 'wave' ? button('conversation',wave,'Conversation only') : kind === 'single' && study.work.get(wave).kind === 'Wave' ? button('wave',wave,'Wave dashboard') : '');
    element('attention').innerHTML = [...study.work].filter(([,w])=>w.needsYou).map(([key,w]) => button('single',key,escape(w.title)+`<small>Waiting · ${escape(study.work.get(w.wave).title)}</small>`)).join('') || '<small>Nothing needs you</small>';
    element('navigation').innerHTML = [...study.work].filter(([key,w])=>key!=='repo'&&!w.needsYou).map(([key,w]) => button(w.kind==='Wave'?'wave-click':'single',key,escape(w.title)+`<small>${w.kind==='Wave'?'Conversation + Tasks':w.state}</small>`,`class="${w.kind==='Wave'?'wave-row':'task-row'}"`)).join('');
    const work = element('work');
    work.className = kind === 'wave' ? 'wave-layout' : '';
    work.innerHTML = study.visible().map(key => {
      const w = study.work.get(key), collapsed = kind === 'wave' && study.expandable && study.collapsed.has(key);
      return `<article class="work-card ${study.selected===key?'active':''}" data-work="${key}"><header class="work-header" ${sessionOnly?'hidden':''}>${button('focus',key,`${escape(w.title)}<small>${w.kind}${w.state?' · '+w.state:''}</small>`,'class="identity"')}${kind==='repo'?button('left',key,'←',`aria-label="Move ${escape(w.title)} earlier"`)+button('right',key,'→',`aria-label="Move ${escape(w.title)} later"`)+(key==='repo'?'':button('remove',key,'Remove')):''}${kind!=='single'?button('single',key,'Open alone'):''}${kind==='repo'&&w.kind==='Wave'?button('wave',key,'Open dashboard'):''}${kind==='wave'&&study.expandable?button('collapse',key,collapsed?'Expand':'Fold'):''}</header>${collapsed?'':`<div class="inner">${pane(key,w)}</div>`}</article>`;
    }).join('');
    paintSelection();
  }
  function paintSelection() {
    document.querySelectorAll('aside [data-key]').forEach(node => {
      const selected = node.dataset.key === study.selected;
      node.classList.toggle('selected', selected);
      if (selected) node.setAttribute('aria-current','true'); else node.removeAttribute('aria-current');
    });
    document.querySelector('.repo').setAttribute('aria-current',study.destination.kind==='repo'?'page':'false');
    document.querySelectorAll('[data-work]').forEach(node=>node.classList.toggle('active',node.dataset.work===study.selected));
  }
  document.addEventListener('click', event => {
    const target = event.target.closest('[data-action]');
    if (!target) return;
    const key = target.dataset.key;
    switch (target.dataset.action) {
      case 'repo': study.home(); break;
      case 'back': study.previous(); break;
      case 'single': study.single(key); break;
      case 'conversation': study.conversation(key); break;
      case 'wave': study.wave(key); break;
      case 'wave-click': study.wave(key); break;
      case 'focus': study.focus(key); paintSelection(); return;
      case 'remove': study.remove(key); break;
      case 'left': study.move(key,-1); break;
      case 'right': study.move(key,1); break;
      case 'content': study.work.get(key).content=study.work.get(key).content==='files'?'conversation':'files'; study.focus(key); break;
      case 'split': study.work.get(key).split=!study.work.get(key).split; study.focus(key); break;
      case 'collapse': study.collapsed.has(key)?study.collapsed.delete(key):study.collapsed.add(key); break;
      case 'complete': study.update('task-theme',study.work.get('task-theme').state==='completed'?'unstarted':'completed'); break;
      case 'error': study.unavailable=!study.unavailable; study.focus(study.selected); break;
      case 'attention': study.work.get('task-design').needsYou=!study.work.get('task-design').needsYou; break;
    }
    render();
  });
  document.addEventListener('input', event => {
    if (event.target.matches('.draft')) study.work.get(event.target.dataset.key)[event.target.dataset.field]=event.target.value;
  });
  document.addEventListener('focusin', event => {
    if (event.target.matches('.draft')) { study.focus(event.target.dataset.key); paintSelection(); }
  });
  document.addEventListener('change', event => {
    if (event.target.id==='add-work' && event.target.value) study.add(event.target.value);
    if (event.target.id==='expandable') study.expandable=event.target.checked;
    render();
  });
  render();
}
