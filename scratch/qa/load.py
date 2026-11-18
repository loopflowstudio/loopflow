from drv import *
import statistics, resource
tag=sys.argv[1]; secs=int(sys.argv[2]) if len(sys.argv)>2 else 20
pid=int(open(f"{ROOT}/{tag}/server.pid").read())
def cpu():
    out=subprocess.run(["ps","-o","time=","-p",str(pid)],capture_output=True,text=True).stdout.strip()
    p=out.split(":"); return float(p[-1])+60*int(p[-2])+(3600*int(p[-3]) if len(p)>2 else 0)
main=Conn(tag); d=main.repo()
items=main.req("w0",{"kind":"wave.plan","directory":d,"wave":"alpha"})["result"]["tasks"]["items"]
idents=[t["task"]["identifier"] for t in items]
subs=[]
for i in range(8):
    c=Conn(tag); c.sub("plan",{"kind":"plan.list","directory":d,"wave":"alpha","task":None,"all":False})
    for j in range(3): c.sub(f"tg{j}",{"kind":"task.get","directory":d,"issue":idents[(i*3+j)%len(idents)]})
    subs.append(c)
time.sleep(5)
stop=time.time()+secs; lat=[]; writes=[0]
def writer():
    w=Conn(tag); i=0
    while time.time()<stop:
        i+=1; r=w.req(f"u{i}",{"kind":"task.update","directory":d,"issue":idents[i%len(idents)],"notes":f"note {i} {time.time()}"},wait=10)
        if r: writes[0]+=1
        time.sleep(0.1)
threading.Thread(target=writer,daemon=True).start()
c0=cpu(); r0=sum(c.n(k) for c in subs for k in list(c.readings)); t0=time.time()
samp=subprocess.Popen(["sample",str(pid),"5","-file",f"{ROOT}/sample-{tag}.txt"],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL) if len(sys.argv)>3 else None
i=0
while time.time()<stop:
    i+=1; a=time.time(); r=main.req(f"p{i}",{"kind":"wave.plan","directory":d,"wave":"alpha"},wait=30)
    lat.append(time.time()-a); time.sleep(0.2)
el=time.time()-t0; c1=cpu(); r1=sum(c.n(k) for c in subs for k in list(c.readings))
if samp: samp.wait()
errs=[m for c in subs+[main] for m in c.other if m.get("type")=="error"]
print(json.dumps({"tag":tag,"build":main.welcome["build"],"seconds":round(el,1),"server_cpu_seconds":round(c1-c0,2),"cpu_percent":round(100*(c1-c0)/el),
  "writes":writes[0],"subscription_readings":r1-r0,"wave.plan_requests":len(lat),"wave.plan_median_ms":round(1000*statistics.median(lat)),"wave.plan_max_ms":round(1000*max(lat)),"errors":errs[:3]}))
