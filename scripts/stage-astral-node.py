"""Stage Node for PSD/modern legacy readers so users need no separate Node install."""
import hashlib,json,os,urllib.request,zipfile,platform
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
VERSION='24.16.0'
URL=f'https://nodejs.org/dist/v{VERSION}/node-v{VERSION}-win-x64.zip'
SHA='edaca9bd58ec8e92037dac4e877d52f6b8f430b81c18b57e264b4e2fb111cd56'
def main():
    if platform.system()!='Windows':raise SystemExit('This staging script pins Windows x64 only.')
    archive=ROOT/f'build/node-v{VERSION}-win-x64.zip';archive.parent.mkdir(exist_ok=True)
    if not archive.exists() or hashlib.sha256(archive.read_bytes()).hexdigest()!=SHA:
        partial=archive.with_suffix('.part')
        try:
            with urllib.request.urlopen(URL,timeout=60) as source,partial.open('wb') as out:
                total=0
                while chunk:=source.read(1024*1024):
                    total+=len(chunk)
                    if total>64*1024*1024:raise RuntimeError('Node archive exceeds budget')
                    out.write(chunk)
            if hashlib.sha256(partial.read_bytes()).hexdigest()!=SHA:raise RuntimeError('Node checksum mismatch')
            os.replace(partial,archive)
        finally:partial.unlink(missing_ok=True)
    destination=ROOT/'build/astral-node';destination.mkdir(exist_ok=True)
    with zipfile.ZipFile(archive) as z:
        for name in ['node.exe','LICENSE','README.md']:
            data=z.read(f'node-v{VERSION}-win-x64/{name}')
            if len(data)>128*1024*1024:raise RuntimeError('Node component exceeds budget')
            (destination/name).write_bytes(data)
    manifest={'version':VERSION,'url':URL,'archive_sha256':SHA,'files':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in destination.iterdir() if p.is_file() and p.name!='provenance.json'}}
    (destination/'provenance.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'staged':str(destination),'archive_sha256':SHA}))
if __name__=='__main__':main()
