"""Exercise a persistent headless document service. --hours 8 is the long stress gate.

This measures engine/MCP memory and latency, not physical stylus or GUI playback.
"""
import argparse,collections,ctypes,hashlib,json,os,platform,queue,statistics,subprocess,threading,time,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def rss(pid):
    if os.name!='nt':
        try:return int(Path(f'/proc/{pid}/statm').read_text().split()[1])*os.sysconf('SC_PAGE_SIZE')
        except OSError:return None
    class Memory(ctypes.Structure):
        _fields_=[('cb',ctypes.c_ulong),('faults',ctypes.c_ulong)]+[(n,ctypes.c_size_t)for n in ['peak','working','peak_paged','paged','peak_nonpaged','nonpaged','pagefile','peak_pagefile','private']]
    kernel=ctypes.windll.kernel32;psapi=ctypes.windll.psapi
    kernel.OpenProcess.argtypes=[ctypes.c_ulong,ctypes.c_int,ctypes.c_ulong];kernel.OpenProcess.restype=ctypes.c_void_p
    kernel.CloseHandle.argtypes=[ctypes.c_void_p];psapi.GetProcessMemoryInfo.argtypes=[ctypes.c_void_p,ctypes.POINTER(Memory),ctypes.c_ulong]
    handle=kernel.OpenProcess(0x410,0,pid)
    if not handle:return None
    try:
        info=Memory();info.cb=ctypes.sizeof(info)
        return info.private if psapi.GetProcessMemoryInfo(handle,ctypes.byref(info),info.cb) else None
    finally:kernel.CloseHandle(handle)
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/astral.exe');parser.add_argument('--hours',type=float,default=8);parser.add_argument('--interval',type=float,default=0.1);parser.add_argument('--output',type=Path,default=ROOT/'artifacts/astral-stress/report.json');args=parser.parse_args()
    if not 0<args.hours<=24 or args.interval<0:raise SystemExit('Use 0 < hours <= 24 and a non-negative interval')
    work=args.output.parent/'workspace';work.mkdir(parents=True,exist_ok=True);args.output.parent.mkdir(parents=True,exist_ok=True)
    child=subprocess.Popen([str(args.binary.resolve()),'--workspace',str(work.resolve()),'mcp','--headless'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,encoding='utf-8',creationflags=0x08000000 if os.name=='nt' else 0)
    messages=queue.Queue();threading.Thread(target=lambda:[messages.put(line) for line in child.stdout],daemon=True).start();sequence=0
    def rpc(method,params):
        nonlocal sequence
        sequence+=1;child.stdin.write(json.dumps({'jsonrpc':'2.0','id':sequence,'method':method,'params':params})+'\n');child.stdin.flush();deadline=time.monotonic()+30
        while True:
            try:reply=json.loads(messages.get(timeout=max(0.01,deadline-time.monotonic())))
            except queue.Empty:raise RuntimeError('MCP response timed out')
            if reply.get('id')!=sequence:continue
            if 'error'in reply:raise RuntimeError(reply['error'])
            return reply['result']
    def tool(name,arguments):
        value=rpc('tools/call',{'name':name,'arguments':arguments})
        if value.get('isError'):raise RuntimeError(value)
        return value.get('structuredContent') or json.loads(next(c['text']for c in value['content']if c['type']=='text'))
    started=time.monotonic();samples=[];latencies=collections.deque(maxlen=4096);operations=0;failure=None;project=None
    try:
        rpc('initialize',{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'Astral stress','version':'0.15.0'}})
        child.stdin.write(json.dumps({'jsonrpc':'2.0','method':'notifications/initialized'})+'\n');child.stdin.flush()
        p=tool('create_project',{'name':'Long stress','width':640,'height':360,'fps':24,'duration':240});project=p['project'];revision=p['revision'];layer=str(uuid.uuid4())
        revision=tool('apply_commands',{'project':project,'expected_revision':revision,'commands':[{'type':'add_layer','layer':{'id':layer,'name':'Ink','visible':True,'locked':False,'opacity':1}}]})['revision']
        while time.monotonic()-started<args.hours*3600:
            before=time.perf_counter();frame=operations%120
            revision=tool('set_drawing_frame',{'project':project,'expected_revision':revision,'layer':layer,'frame':frame,'strokes':[{'points':[{'x':100,'y':180+frame%10},{'x':320,'y':100+frame%20},{'x':540,'y':180}],'width':8,'color':[30,50,80,255],'pressures':[0.2,1,0.4]}]})['revision'];latencies.append((time.perf_counter()-before)*1000)
            if operations%5==0:
                revision=tool('history',{'project':project,'expected_revision':revision,'redo':False})['revision'];revision=tool('history',{'project':project,'expected_revision':revision,'redo':True})['revision']
            if operations%10==0:tool('render_preview',{'project':project,'frame':frame,'width':640,'height':360,'path':'latest.png'})
            if operations%20==0:tool('save_project',{'project':project,'path':'Stress.astral'})
            elapsed=time.monotonic()-started
            if not samples or elapsed-samples[-1]['seconds']>=15:
                samples.append({'seconds':elapsed,'private_bytes':rss(child.pid)});progress={'seconds':elapsed,'operations':operations,'last_memory':samples[-1]};args.output.with_suffix('.progress.json').write_text(json.dumps(progress),encoding='utf-8')
            operations+=1
            if args.interval:time.sleep(args.interval)
        tool('save_project',{'project':project,'path':'Stress.astral'})
        inspected=tool('inspect_project',{'project':project})['document'];assert inspected['revision']==revision
    except Exception as error:failure=str(error)
    finally:
        child.stdin.close()
        try:child.wait(timeout=10)
        except subprocess.TimeoutExpired:child.kill();child.wait()
    elapsed=time.monotonic()-started;valid=[s['private_bytes']for s in samples if s['private_bytes']is not None];window=max(1,min(40,len(valid)//3));growth=statistics.median(valid[-window:])-statistics.median(valid[:window])if valid else None
    stable=growth is not None and growth<=max(64*1024*1024,statistics.median(valid[:window])*0.2)
    report={'kind':'headless_engine_stress','version':'0.15.0','binary_sha256':hashlib.sha256(args.binary.read_bytes()).hexdigest(),'platform':platform.system(),'architecture':platform.machine(),'duration_seconds':elapsed,'operations':operations,'failure':failure,'samples':samples,'retained_latency_samples':len(latencies),'command_p95_ms':sorted(latencies)[int((len(latencies)-1)*0.95)]if latencies else None,'memory_growth_bytes':growth,'bounded_growth_heuristic':stable,'eight_hour_engine_gate_passed':elapsed>=8*3600 and failure is None and stable,'physical_input_or_gui_validation':False,'production_certified':False}
    args.output.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8');print(json.dumps({k:v for k,v in report.items()if k!='samples'}))
    if failure:raise SystemExit(1)
if __name__=='__main__':main()
