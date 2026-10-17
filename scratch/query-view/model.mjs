// Disposable interaction model. All Work and terminal output below is illustrative.
export const initialWork = [
  {id:'product',kind:'wave',title:'Product',description:'A calm place to see, shape, and finish the work.',repo:'loopflow',parent:null},
  {id:'infrastructure',kind:'wave',title:'Infrastructure',description:'Keep the tools dependable. Keep the work moving.',repo:'loopflow',parent:null},
  {id:'intelligence',kind:'wave',title:'Intelligence',description:'Make each conversation more useful than the last.',repo:'loopflow',parent:null},
  {id:'LF-101',kind:'task',title:'Query-based views',wave:'product',started:true,finished:false,status:'available',provider:'Codex',branch:'query-views',request:'Show me the work that needs a decision.',reply:'The view is ready to try.\n\nA query chooses the Work. Each tile keeps its own conversation.\n\nWhich feels clearer: editing the expression, or choosing the scope first?',tool:'Waiting for your review'},
  {id:'LF-102',kind:'task',title:'Terminal colors',wave:'product',started:true,finished:false,status:'running',provider:'Claude',branch:'terminal-colors',request:'Keep the terminal and app chrome in the same palette.',reply:'The selection now stays separate from error colors.\n\nChecking contrast in both appearances and preserving the terminal’s original syntax colors.',tool:'✓ 14 palette checks passed'},
  {id:'LF-103',kind:'task',title:'Search result context',wave:'product',started:true,finished:false,status:'running',provider:'Codex',branch:'search-context',request:'Make the result understandable before opening it.',reply:'Added the surrounding lines and their Work owner.\n\nThe next check covers a result whose Task moved between Waves.',tool:'↳ Running the focused suite'},
  {id:'LF-104',kind:'task',title:'Retain Session identity',wave:'infrastructure',started:true,finished:false,status:'available',provider:'Codex',branch:'session-retention',request:'Come back to the same conversation after switching views.',reply:'The same Session stays attached when its view changes.\n\nThe remaining decision is whether restoring a hidden tile should also move keyboard focus.',tool:'Waiting for a decision'},
  {id:'LF-105',kind:'task',title:'Recover an interrupted Flow',wave:'infrastructure',started:true,finished:false,status:'paused',provider:'Claude',branch:'flow-recovery',request:'Keep successful effects when a step is retried.',reply:'The captured Process is retained. A retry only runs the remaining step.\n\nPaused while the new dependency is being reviewed.',tool:'Paused · still unfinished'},
  {id:'LF-106',kind:'task',title:'Remember the last useful context',wave:'intelligence',started:true,finished:false,status:'idle',provider:'Codex',branch:'context-memory',request:'Preserve the decision without copying the whole transcript.',reply:'The context update is prepared.\n\nThe short note preserves the accepted direction and the evidence that changed it.',tool:'✓ Focused checks passed · Task remains open'},
  {id:'LF-108',kind:'task',title:'A better empty state',wave:'product',started:false,finished:false,status:'backlog',provider:null,branch:null,request:'Help someone find useful Work in an empty view.',reply:'This Task has not started. Selecting it does not create a Session.',tool:'No Session yet'},
  {id:'LF-109',kind:'task',title:'Repair repository links',wave:'infrastructure',started:true,finished:true,status:'done',provider:'Codex',branch:'repository-links',request:'Keep links stable across checkout moves.',reply:'The change is delivered and the Task is complete.',tool:'Completed'},
];

export const presets = {
  repo:{name:'Repository',query:'repo:loopflow (kind:wave or state:open)',scope:'repo'},
  product:{name:'Product',query:'wave:product state:open',scope:'product'},
  attention:{name:'Needs you',query:'state:available',scope:'repo'},
  overlap:{name:'Product + Needs you',query:'query:product or query:needs-you',scope:'repo'},
  picked:{name:'A hand-picked pair',query:'task:LF-101 or task:LF-104',scope:'repo'},
};

export const savedQueries = {'needs-you':'state:available',product:'wave:product state:open'};
export function openTask(work){return work.kind === 'task' && work.started && !work.finished;}
export function priority(work){return work.kind === 'wave' ? 9 : ({available:0,blocked:0,running:1,paused:2,idle:3,backlog:4,done:5}[work.status] ?? 6);}

export function parseQuery(source) {
  const tokens=[]; let rest=source.trim();
  while(rest){
    const match=rest.match(/^(\(|\)|\b(?:or|and)\b|[a-z]+:(?:"[^"]*"|[^\s()]+))/i);
    if(!match) throw new Error(`Expected a filter near “${rest.slice(0,22)}”.`);
    tokens.push(match[0]);rest=rest.slice(match[0].length).trimStart();
  }
  if(!tokens.length) throw new Error('Enter a scope or a filter.');
  let at=0;
  function atom(){
    const token=tokens[at++];
    if(token==='('){const node=or();if(tokens[at++]!==')')throw new Error('Close the parenthesis.');return node;}
    if(!token || token===')' || /^(and|or)$/i.test(token))throw new Error('Expected a filter.');
    const colon=token.indexOf(':');const key=token.slice(0,colon).toLowerCase();const value=token.slice(colon+1).replace(/^"|"$/g,'');
    if(!['repo','wave','task','query','kind','state'].includes(key))throw new Error(`Unknown filter “${key}”.`);
    if(!value)throw new Error(`Give ${key}: a value.`);
    if(key==='state'&&!['open','available','running','blocked','paused','idle','backlog','done'].includes(value))throw new Error(`Unknown state “${value}”.`);
    if(key==='kind'&&!['wave','task'].includes(value))throw new Error('Use kind:wave or kind:task.');
    return {key,value};
  }
  function and(){let node=atom();while(at<tokens.length&&tokens[at]!==')'&&tokens[at].toLowerCase()!=='or'){if(tokens[at].toLowerCase()==='and')at++;node={op:'and',left:node,right:atom()};}return node;}
  function or(){let node=and();while(tokens[at]?.toLowerCase()==='or'){at++;node={op:'or',left:node,right:and()};}return node;}
  const node=or();if(at!==tokens.length)throw new Error('Unexpected closing parenthesis.');return node;
}

export function resolveQuery(source, work, catalog=savedQueries) {
  const sorted=[...work].sort((a,b)=>priority(a)-priority(b)||a.id.localeCompare(b.id));
  const waves=new Map(work.filter(w=>w.kind==='wave').map(w=>[w.id,w]));
  function belongs(wave,scope){const seen=new Set();while(wave&&!seen.has(wave)){if(wave===scope)return true;seen.add(wave);wave=waves.get(wave)?.parent;}return false;}
  function evaluate(node,path,seen){
    if(node.op==='or')return [...evaluate(node.left,path+'L',seen),...evaluate(node.right,path+'R',seen)];
    if(node.op==='and'){const right=new Set(evaluate(node.right,path+'F',seen).map(x=>x.work.id));return evaluate(node.left,path+'A',seen).filter(x=>right.has(x.work.id));}
    if(node.key==='query'){
      if(!(node.value in catalog))throw new Error(`No saved query named “${node.value}”.`);
      if(seen.includes(node.value))throw new Error('Saved queries cannot include themselves.');
      return evaluate(parseQuery(catalog[node.value]),path+'Q'+node.value,[...seen,node.value]);
    }
    if(node.key==='wave'&&!waves.has(node.value))throw new Error(`No Wave named “${node.value}” in the sample Work.`);
    return sorted.filter(w=>{
      switch(node.key){
        case 'repo':return node.value==='loopflow';
        case 'kind':return w.kind===node.value;
        case 'task':return w.kind==='task'&&w.id.toLowerCase()===node.value.toLowerCase();
        case 'wave':return belongs(w.kind==='task'?w.wave:w.parent,node.value);
        case 'state':return node.value==='open'?openTask(w):w.kind==='task'&&w.status===node.value;
      }
      return false;
    }).map(w=>({id:`${path}:${w.id}`,work:w}));
  }
  return evaluate(parseQuery(source),'m',[]);
}

export function tileTree(ids,depth=0){
  if(!ids.length)return null;
  if(ids.length===1)return {member:ids[0]};
  const cut=Math.ceil(ids.length/2);
  return {axis:depth%2?'h':'v',ratio:.5,first:tileTree(ids.slice(0,cut),depth+1),second:tileTree(ids.slice(cut),depth+1)};
}
export function treeMembers(tree){return !tree?[]:tree.member?[tree.member]:[...treeMembers(tree.first),...treeMembers(tree.second)];}
export function pruneTree(tree,exclude){
  if(!tree)return null;if(tree.member)return exclude.has(tree.member)?null:tree;
  const first=pruneTree(tree.first,exclude),second=pruneTree(tree.second,exclude);
  return first&&second?{...tree,first,second}:first||second;
}
export function projection(tree,members,hidden){
  const lookup=new Map(members.map(m=>[m.id,m]));const seen=new Map(),suppressed=new Map();
  for(const id of treeMembers(tree)){
    const item=lookup.get(id);if(!item||hidden.has(id))continue;
    const key=`${item.work.kind}:${item.work.id}`;
    if(seen.has(key))suppressed.set(id,seen.get(key));else seen.set(key,id);
  }
  return {tree:pruneTree(tree,new Set([...hidden,...suppressed.keys()])),suppressed};
}
export function reconcile(view,members){
  const ids=members.map(m=>m.id),existing=new Set(treeMembers(view.tree));
  if(!view.touched)view.tree=tileTree(ids);
  else{
    const removed=new Set([...existing].filter(id=>!ids.includes(id)));
    view.tree=pruneTree(view.tree,removed);
    const arrivals=ids.filter(id=>!existing.has(id));
    if(arrivals.length){const inbox=tileTree(arrivals);view.tree=view.tree?{axis:'v',ratio:.72,first:view.tree,second:inbox}:inbox;}
  }
  view.members=members;view.hidden=new Set([...view.hidden].filter(id=>ids.includes(id)));
}
export function validateTree(tree,ids){
  function walk(node){if(!node||typeof node!=='object')throw new Error('A node must be a member or a split.');
    if('member' in node){if(Object.keys(node).length!==1||typeof node.member!=='string')throw new Error('A leaf contains only its member ID.');return;}
    if(!['v','h'].includes(node.axis)||!Number.isFinite(node.ratio)||node.ratio<.1||node.ratio>.9)throw new Error('Use axis v/h and a ratio from 0.1 to 0.9.');
    if(Object.keys(node).some(k=>!['axis','ratio','first','second'].includes(k)))throw new Error('A split contains axis, ratio, first and second.');walk(node.first);walk(node.second);
  }
  if(!ids.length&&tree===null)return;
  walk(tree);const actual=treeMembers(tree);
  if(actual.length!==ids.length||new Set(actual).size!==actual.length||actual.some(id=>!ids.includes(id)))throw new Error('Keep every member exactly once, including hidden matches.');
}
export function swapMembers(tree,a,b){if(!tree)return tree;if(tree.member)return {member:tree.member===a?b:tree.member===b?a:tree.member};return {...tree,first:swapMembers(tree.first,a,b),second:swapMembers(tree.second,a,b)};}
