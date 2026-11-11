"""Excluded scoped wall timers and bound-SQL attribution on exact commit exports."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from iteration3 import ROOT, COMMITS, c, configure

TIMER = '''
#[derive(Debug)]
pub(crate) struct MeasureRead(&'static str, std::time::Instant);
pub(crate) fn measure_read(name: &'static str) -> MeasureRead {
    MeasureRead(name, std::time::Instant::now())
}
impl Drop for MeasureRead {
    fn drop(&mut self) {
        let ns = self.1.elapsed().as_nanos();
        if std::env::var_os("MEASURE_READ_TIMING").is_some() {
            eprintln!("MEASURE_READ {} {}", self.0, ns);
        }
    }
}
'''


def build():
    manifest = json.loads((ROOT / 'manifest.json').read_text())
    source = ROOT / 'source'
    env = {k:v for k,v in os.environ.items() if not k.startswith('LF_')}
    env.update(CARGO_TARGET_DIR=str(c.REPO/'target'), LOOPFLOW_MIGRATION_ORDER=str(ROOT/'migration-order.txt'), LOOPFLOW_BUILD_PROVENANCE='development')
    for side in ('after1','after2'):
        with (ROOT/'source.tar').open('wb') as archive:
            subprocess.run(['git','archive',COMMITS[side]],cwd=c.REPO,stdout=archive,check=True)
        subprocess.run(['tar','-xf',str(ROOT/'source.tar'),'-C',str(source)],check=True)
        perf = source/'rust/loopflow/src/performance.rs'
        original = perf.read_text()
        replacement = original.replace('pub(crate) fn observe_sqlite(connection: &rusqlite::Connection) {', 'pub(crate) fn observe_sqlite(connection: &rusqlite::Connection) {\n    session_list_trace(connection);')
        assert replacement != original
        perf.write_text(replacement + '\n' + Path(__file__).with_name('trace_bound.rs').read_text() + TIMER)
        for name, signature, label in (
            ('repository.rs','pub fn discover(path: &Path) -> Result<Self, RepoIdError> {','repository_discover'),
            ('ops/planning_client.rs','pub(crate) fn repository_scope(repo: &Path) -> OpsResult<String> {','repository_scope'),
            ('ops/linear_observe.rs','async fn check_scope(store: &Store, repository: &str, fresh_for: i64) -> OpsResult<bool> {','check_scope'),
        ):
            path = source/'rust/loopflow/src'/name
            original = path.read_text()
            assert original.count(signature)==1
            path.write_text(original.replace(signature, signature+'\n    let _measure = crate::performance::measure_read("'+label+'");'))
        label = side+'-trace'
        print('Building '+label,flush=True)
        with (ROOT/(label+'-build.log')).open('w') as log:
            subprocess.run(['cargo','build','--release','-p','loopflow','--bin','lf'],cwd=source,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,check=True)
        shutil.copy2(c.REPO/'target/release/lf',ROOT/label)
        manifest['builds'][label] = dict(commit=COMMITS[side],sha256=hashlib.sha256((ROOT/label).read_bytes()).hexdigest(),instrumentation='repository_scope, check_scope and RepoId::discover wall guards; optional bound-SQL trace')
        (ROOT/'manifest.json').write_text(json.dumps(manifest,indent=2))
        print('Built '+label,flush=True)


def profile():
    configure('task')
    for side in ('after1','after2'):
        for sql in (False,True):
            c.activate(side,True)
            env = c.environment()
            ms,data,_ = c.cli('task',env)
            warmup = dict(ms=ms,**c.validate('task',data))
            label = ('sql-' if sql else 'scope-')+side
            env['MEASURE_READ_TIMING']='1'
            if sql:
                env['SESSION_LIST_TRACE']=str(ROOT/(label+'-trace.jsonl'))
                Path(env['SESSION_LIST_TRACE']).unlink(missing_ok=True)
            ms,data,stderr = c.cli('task',env)
            (ROOT/(label+'.json')).write_text(json.dumps(data))
            timers={}
            for line in stderr.splitlines():
                if line.startswith('MEASURE_READ '):
                    _,name,ns = line.split()
                    timers.setdefault(name,[]).append(int(ns)/1e6)
            row=dict(label=label,side=side,workload='task',ms=ms,errors=0,warmup=warmup,**c.validate('task',data),timers={k:dict(calls=len(v),total_ms=sum(v),range_ms=[min(v),max(v)]) for k,v in timers.items()})
            assert row['full_digest']==warmup['full_digest']
            with (ROOT/'attribution.jsonl').open('a') as out: out.write(json.dumps(row)+'\n')
            print(json.dumps(row),flush=True)


if __name__=='__main__':
    if sys.argv[1]=='build': build()
    else: profile()
