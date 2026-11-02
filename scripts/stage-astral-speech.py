"""Stage the pinned official Rhubarb runtime, models, provenance and full notices."""
import hashlib,json,os,platform,shutil,urllib.request,zipfile
from pathlib import Path,PurePosixPath
ROOT=Path(__file__).resolve().parents[1]
URL='https://github.com/DanielSWolf/rhubarb-lip-sync/releases/download/v1.14.0/Rhubarb-Lip-Sync-1.14.0-Windows.zip'
SHA='62fa416a8d5e382a3828ee4bef358ce520d0b4cabdeaea75a7ac266d098d1fe3'
def main():
    if platform.system()!='Windows':raise SystemExit('This pinned runtime is Windows x64. Stage the appropriate official runtime separately for other platforms.')
    archive=ROOT/'build/rhubarb-1.14.0-windows.zip';archive.parent.mkdir(exist_ok=True)
    if not archive.exists() or hashlib.sha256(archive.read_bytes()).hexdigest()!=SHA:
        partial=archive.with_suffix('.part')
        try:
            with urllib.request.urlopen(URL,timeout=60) as source,partial.open('wb') as target:
                total=0
                while chunk:=source.read(1024*1024):
                    total+=len(chunk)
                    if total>100*1024*1024:raise RuntimeError('Speech runtime exceeds download budget')
                    target.write(chunk)
            if hashlib.sha256(partial.read_bytes()).hexdigest()!=SHA:raise RuntimeError('Speech runtime checksum mismatch')
            os.replace(partial,archive)
        finally:partial.unlink(missing_ok=True)
    destination=ROOT/'build/rhubarb';destination.mkdir(exist_ok=True)
    with zipfile.ZipFile(archive) as z:
        total=0
        for entry in z.infolist():
            path=PurePosixPath(entry.filename)
            if entry.is_dir():continue
            if '..' in path.parts or path.is_absolute() or len(path.parts)<2:raise RuntimeError('Unsafe runtime archive path')
            relative=Path(*path.parts[1:])
            if relative.parts[0]!='res' and relative.name not in ['rhubarb.exe','LICENSE.md','README.md','CHANGELOG.md']:continue
            total+=entry.file_size
            if total>128*1024*1024:raise RuntimeError('Unpacked runtime exceeds budget')
            target=destination/relative;target.parent.mkdir(parents=True,exist_ok=True)
            with z.open(entry) as source,target.open('wb') as output:shutil.copyfileobj(source,output)
    files={p.relative_to(destination).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in destination.rglob('*') if p.is_file() and p.name!='provenance.json'}
    if files.get('rhubarb.exe')!='9e289c6b5939ef6b306a61e8105ec721fd8d52b3ce950d08891ea3cc7df5718d':raise RuntimeError('Recognizer executable checksum mismatch')
    if 'LICENSE.md' not in files:raise RuntimeError('Recognizer/model notices missing')
    (destination/'provenance.json').write_text(json.dumps({'version':'1.14.0','url':URL,'archive_sha256':SHA,'files':files},indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'staged':str(destination),'files':len(files),'archive_sha256':SHA}))
if __name__=='__main__':main()
