(function (root) {
  'use strict';
  const seed = [
    ['489','Arrange navigation','Product','Waiting','The navigation foundation is ready. Which return gesture feels most natural?','Review the navigation studies'],
    ['497','Selection and arrangement','Product','Waiting','The query keeps membership; the layout keeps positions. The state projection now makes both visible.','Review the query interaction'],
    ['453','Know when work needs you','Product','Working','Tracing the attention event through the Work stream. The consumer now receives one coherent update.','Checking event ordering'],
    ['487','Terminal feedback and color','Product','Working','The foreground now keeps its contrast in both appearances. Testing the ANSI palette against the shared roles.','Running appearance checks'],
    ['462','Work-first navigation','Product','Working','Preserved the selected Task while the sidebar reading refreshed. Returning to the Wave keeps the same pane.','Checking retained selection'],
    ['478','Readable Task history','Product','Working','Grouped the captured steps by their Flow. Historical results stay separate from current execution.','Checking the history fixture'],
    ['481','Files beside conversations','Product','Working','The draft belongs to its Task workspace. Switching Sessions no longer reconstructs the editor document.','Testing draft identity'],
    ['495','Repository windows','Product','Working','Each repository has its own title and Window menu entry. Command-backtick cycles between them.','Inspecting window routing'],
    ['416','Seamless Task Sessions','Infrastructure','Waiting','The process survives its viewer. Reopening resolves the same Session before attaching the terminal.','Review the continuity scenario'],
    ['415','Session transport','Infrastructure','Working','The relay preserves input and output boundaries. A disconnected display does not stop the process.','Testing reconnect'],
    ['422','Program status fidelity','Infrastructure','Working','Reported status stays distinct from inferred attention. The terminal title uses its existing owner.','Checking status frames'],
    ['494','Reliable Work reads','Infrastructure','Working','Partial evidence keeps the last known members. Missing information never becomes an empty selection.','Testing incomplete readings']
  ];
  class Study {
    constructor(set = 'navigation', variant = 'a') {
      this.set = set; this.variant = variant;
      this.tasks = seed.map(([id,title,wave,status,output,next]) => ({id,title,wave,status,output,next,draft:'',open:true,session:`session-${id}`,pane:`pane-${id}`}));
      this.route = {kind:'wave',id:'Product'}; this.history=[]; this.forward=[];
      this.beside = null; this.lastContainer = {...this.route}; this.lastTask = null;
      this.scrolls = {}; this.hidden = new Set(); this.touched = false; this.inbox=[];
      this.order = this.tasks.map(t=>t.id); this.notice=''; this.retainedProminent=new Set();
    }
    task(id) { return this.tasks.find(t=>t.id===id); }
    key(route=this.route) { return `${route.kind}:${route.id}`; }
    label(route=this.route) {
      if(route.kind==='repo') return 'loopflow';
      if(route.kind==='task') return `LOO-${route.id}`;
      return route.id;
    }
    members(route=this.route) {
      if(route.kind==='task') return [this.task(route.id)];
      return this.tasks.filter(t=>t.open && (route.kind==='wave' ? t.wave===route.id : route.kind==='query' ? t.status==='Waiting' : true));
    }
    ordered(route=this.route) {
      const members=this.members(route).filter(t=>!this.hidden.has(t.id));
      return members.sort((a,b)=> this.touched ? this.order.indexOf(a.id)-this.order.indexOf(b.id) : (a.status==='Waiting'?0:1)-(b.status==='Waiting'?0:1) || this.order.indexOf(a.id)-this.order.indexOf(b.id));
    }
    visit(route, alongside=false) {
      if(this.key(route)===this.key()) return;
      this.history.push({route:{...this.route},beside:this.beside?{...this.beside}:null});this.forward=[];
      if(route.kind==='task') {
        this.lastTask=route.id;
        if(alongside) this.beside=this.beside || (this.route.kind==='task'?{...this.lastContainer}:{...this.route});
        else this.beside=null;
      } else {this.beside=null;this.lastContainer={...route};}
      this.route={...route};
    }
    back() {
      const visit=this.history.pop(); if(!visit)return;
      this.forward.push({route:{...this.route},beside:this.beside});
      this.route=visit.route;this.beside=visit.beside;
      if(this.route.kind==='task')this.lastTask=this.route.id;
      else if(this.route.kind!=='repo')this.lastContainer={...this.route};
    }
    ahead() {
      const visit=this.forward.pop();if(!visit)return;
      this.history.push({route:{...this.route},beside:this.beside});this.route=visit.route;this.beside=visit.beside;
      if(this.route.kind==='task')this.lastTask=this.route.id;
      else if(this.route.kind!=='repo')this.lastContainer={...this.route};
    }
    closeBeside() {if(this.beside)this.visit({...this.beside});}
    hide(id) {this.hidden.add(id);this.notice=`LOO-${id} is hidden; it remains a member.`;}
    restore(id) {this.hidden.delete(id);this.notice=`LOO-${id} restored to its retained position.`;}
    prominent(id) { return this.touched ? this.retainedProminent.has(id) : this.task(id).status==='Waiting'; }
    resetArrangement() { this.touched=false;this.inbox=[];this.retainedProminent.clear(); }
    move(id,before) {
      const index=this.order.indexOf(before); if(id===before||index<0)return;
      if(!this.touched){
        this.order=this.tasks.slice().sort((a,b)=>(a.status==='Waiting'?0:1)-(b.status==='Waiting'?0:1)||this.order.indexOf(a.id)-this.order.indexOf(b.id)).map(t=>t.id);
        this.retainedProminent=new Set(this.tasks.filter(t=>t.status==='Waiting').map(t=>t.id));
      }
      this.order=this.order.filter(x=>x!==id);this.order.splice(this.order.indexOf(before),0,id);
      this.inbox=this.inbox.filter(x=>x!==id);this.touched=true;this.notice='Arrangement retained. Attention changes will not move these Tasks.';
    }
    arrive() {
      if(this.task('506')) {this.notice='The sample arrival is already present.';return;}
      const t={id:'506',title:'A new matching Task',wave:'Product',status:'Working',output:'This Task just started and now matches the Wave. Its conversation belongs to its own workspace.',next:'Preparing the focused check',draft:'',open:true,session:'session-506',pane:'pane-506'};
      this.tasks.push(t);this.order.push(t.id);
      if(this.touched)this.inbox.push(t.id);
      this.notice=this.touched?'A new match arrived. Existing positions are unchanged.':'Auto layout includes the new matching Task.';
    }
    depart() {
      const t=this.task('487');t.open=!t.open;this.notice=t.open?'LOO-487 reopened; its draft and pane are retained.':'LOO-487 completed and left the selection; its workspace is retained.';
    }
    flipAttention() {
      const t=this.task('453');t.status=t.status==='Waiting'?'Working':'Waiting';
      this.notice=this.touched?'Attention changed. Your arrangement stays put.':'Attention changed. Auto layout sorts waiting Tasks first.';
    }
  }
  root.ArrangeStudy=Study;
  if(typeof module!=='undefined')module.exports=Study;
})(typeof window!=='undefined'?window:globalThis);
