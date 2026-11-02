"""Fail closed for production claims. Evidence must bind to this exact binary hash."""
import argparse,hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def evaluate(gates,evidence,digest):
    pending=[]
    for gate in gates['gates']:
        entry=evidence.get('gates',{}).get(gate['id'],{})
        path=Path(entry.get('report',''))
        bound=entry.get('binary_sha256')==digest and evidence.get('version')==gates['version']
        valid=entry.get('status')=='passed' and bound and path.is_file() and entry.get('report_sha256')==hashlib.sha256(path.read_bytes()).hexdigest()
        if gate['id']=='memory_8h':valid=valid and entry.get('engine_seconds',0)>=28800 and entry.get('desktop_seconds',0)>=28800
        if not valid:pending.append({'id':gate['id'],'requirement':gate['requirement']})
    return {'version':gates['version'],'production_ready':not pending,'pending':pending,'binary_sha256':digest}
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--evidence',type=Path);parser.add_argument('--allow-candidate',action='store_true');args=parser.parse_args()
    gates=json.loads((ROOT/'platforms/release/production-gates.json').read_text(encoding='utf-8'));evidence=json.loads(args.evidence.read_text(encoding='utf-8'))if args.evidence else {};digest=hashlib.sha256(args.binary.read_bytes()).hexdigest();report=evaluate(gates,evidence,digest);print(json.dumps(report,indent=2))
    if not report['production_ready'] and not args.allow_candidate:raise SystemExit(1)
if __name__=='__main__':main()
