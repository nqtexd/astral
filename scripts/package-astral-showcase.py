"""Package original editable studies and previews, never runtime/user history."""
from pathlib import Path
import argparse,hashlib,json,tomllib,zipfile
ROOT=Path(__file__).resolve().parents[1]
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--without-video',action='store_true');args=parser.parse_args()
    version=tomllib.loads((ROOT/'Cargo.toml').read_text(encoding='utf-8'))['workspace']['package']['version']
    source=ROOT/'docs/showcase';output=ROOT/f'release/Astral-{version}-showcase.zip';output.parent.mkdir(exist_ok=True)
    files=[(source/'README.md','README.md')]
    for name in ['contact','soft','voice','raster-stress']:files.append((source/f'projects/{name}.astral',f'projects/{name}.astral'))
    files.extend((file,'projects/assets/'+file.name)for file in sorted((source/'projects/assets').glob('*.wav')))
    if not args.without_video:
        files.extend((source/f'projects/renders/{name}.mp4',f'previews/{name}.mp4')for name in ['contact','soft','voice'])
    for file,_ in files:
        if not file.is_file():raise SystemExit('Missing showcase file: '+str(file))
    manifest={relative:hashlib.sha256(file.read_bytes()).hexdigest()for file,relative in files}
    with zipfile.ZipFile(output,'w',compression=zipfile.ZIP_DEFLATED)as archive:
        for file,relative in files:archive.write(file,'Astral-showcase/'+relative)
        archive.writestr('Astral-showcase/SHA256SUMS.txt',''.join(f'{digest}  {name}\n'for name,digest in manifest.items()))
    print(json.dumps({'file':str(output),'bytes':output.stat().st_size,'sha256':hashlib.sha256(output.read_bytes()).hexdigest(),'files':len(files)}))
if __name__=='__main__':main()
