"""Report release gaps; --final refuses publication without parity and device evidence."""
from pathlib import Path
import argparse,hashlib,json,re,sys

ROOT=Path(__file__).resolve().parents[1]

def inspect(manifest,root=ROOT):
    failures=[]
    if not manifest.get('inventory_complete'):failures.append('Full Debut edition feature inventory has not been signed off')
    if '-' in manifest['version']:failures.append('Version is a development/prerelease version')
    for feature in manifest['features']:
        if feature['status']!='implemented':failures.append('Feature incomplete: '+feature['id'])
        elif not feature.get('evidence') or not (root/feature['evidence']).is_file():failures.append('Missing feature evidence: '+feature['id'])
    for platform in manifest['platforms']:
        if platform['status']!='verified':failures.append('Platform not verified: '+platform['id']);continue
        path=platform.get('evidence')
        try:
            evidence_path=(root/path).resolve()
            if not evidence_path.is_relative_to(root.resolve()):raise ValueError('Evidence escapes repository')
            evidence=json.loads(evidence_path.read_text(encoding='utf-8'))
            if evidence.get('version')!=manifest['version'] or evidence.get('platform')!=platform['id'] or evidence.get('passed')is not True or evidence.get('physical_device_tested')is not True:raise ValueError('Device/version evidence mismatch')
            artifact=(root/evidence['artifact']).resolve()
            if not artifact.is_relative_to(root.resolve())or hashlib.sha256(artifact.read_bytes()).hexdigest()!=evidence['sha256']:raise ValueError('Artifact checksum mismatch')
            if platform['id']in ['iphone','ipad','android_phone','android_tablet']and evidence.get('production_signed')is not True:raise ValueError('Mobile artifact is not production signed')
        except (OSError,ValueError,TypeError,KeyError)as error:failures.append('Invalid platform evidence: '+platform['id']+' / '+str(error))
    for gate,passed in manifest['publication'].items():
        if passed is not True:failures.append('Publication gate incomplete: '+gate)
    return {'product':manifest['product'],'version':manifest['version'],'final_release_ready':not failures,'blockers':failures,
            'implemented_features':sum(f['status']=='implemented'for f in manifest['features']),'confirmed_feature_requirements':len(manifest['features'])}

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--final',action='store_true');parser.add_argument('--output',type=Path)
    args=parser.parse_args();report=inspect(json.loads((ROOT/'platforms/release/readiness.json').read_text(encoding='utf-8')))
    if args.output:args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(report,indent=2))
    if args.final and not report['final_release_ready']:sys.exit(1)
if __name__=='__main__':main()
