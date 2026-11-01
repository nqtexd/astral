"""Measure actual CPU/GPU exports. No universal speedup is inferred."""
from pathlib import Path
import argparse,hashlib,json,os,platform,statistics,subprocess,time
ROOT=Path(__file__).resolve().parents[1]
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=ROOT/'target/release/astral.exe')
    parser.add_argument('--repeats',type=int,default=3)
    parser.add_argument('--width',type=int,default=1920)
    parser.add_argument('--height',type=int,default=1080)
    parser.add_argument('--output',type=Path,default=ROOT/'artifacts/renderer-benchmark/report.json')
    args=parser.parse_args();work=ROOT/'docs/showcase/projects';runs=[]
    for repeat in range(args.repeats):
        for scene in ['contact','soft','voice','raster-stress']:
            for renderer in (['cpu','gpu'] if repeat%2==0 else ['gpu','cpu']):
                relative=f'renders/benchmark-{scene}-{renderer}-{repeat}.mp4'
                tick=time.perf_counter()
                result=subprocess.run([str(args.binary),'--workspace',str(work),'export',f'{scene}.astral',relative,'--renderer',renderer,'--width',str(args.width),'--height',str(args.height),'--fps','30'],cwd=ROOT,capture_output=True,text=True,encoding='utf-8',check=True)
                value=json.loads(result.stdout);runs.append({'scene':scene,'renderer':renderer,'repeat':repeat,'wall_seconds':time.perf_counter()-tick,'metrics':value['metrics'],'encoder':value['encoder'],'warnings':value.get('warnings',[])})
                print(json.dumps(runs[-1]),flush=True)
    summary=[]
    for scene in ['contact','soft','voice','raster-stress']:
        cpu=statistics.median(r['metrics']['total_seconds']for r in runs if r['scene']==scene and r['renderer']=='cpu')
        gpu=statistics.median(r['metrics']['total_seconds']for r in runs if r['scene']==scene and r['renderer']=='gpu')
        summary.append({'scene':scene,'cpu_seconds':cpu,'gpu_seconds':gpu,'cpu_over_gpu_ratio':cpu/gpu})
    report={'version':subprocess.check_output([str(args.binary),'--version'],text=True).strip(),'platform':platform.platform(),'output':[args.width,args.height,30],'repeats':args.repeats,'hardware_encoding':False,'source_sha256':{s:hashlib.sha256((work/f'{s}.astral').read_bytes()).hexdigest()for s in ['contact','soft','voice','raster-stress']},'summary':summary,'runs':runs,'limits':'One PC, four short original studies, libx264 matched settings. CPU render_seconds sums workers; GPU render_seconds is serial elapsed. Compare total_seconds. No physical AMD/Apple/mobile measurements.'}
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8');print(json.dumps({'summary':summary,'report':str(args.output)}))
if __name__=='__main__':main()
