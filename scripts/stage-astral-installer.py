"""Stage the official signed Inno compiler in private portable mode (no system install)."""
import hashlib,json,os,subprocess,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
URL='https://github.com/jrsoftware/issrc/releases/download/is-7_1_1/innosetup-7.1.1-x64.exe'
SHA='8a5a6711c3766a6e5fb74f280ba0e3551e719ce94538d901fdbd1f8f9920b3f0'
def main():
    if os.name!='nt':raise SystemExit('The Windows installer compiler requires Windows.')
    package=ROOT/'build/innosetup-7.1.1-x64.exe';package.parent.mkdir(exist_ok=True)
    if not package.exists() or hashlib.sha256(package.read_bytes()).hexdigest()!=SHA:
        partial=package.with_suffix('.part')
        try:
            with urllib.request.urlopen(URL,timeout=60) as source,partial.open('wb') as output:
                total=0
                while chunk:=source.read(1024*1024):
                    total+=len(chunk)
                    if total>32*1024*1024:raise RuntimeError('Installer compiler download exceeds budget')
                    output.write(chunk)
            if hashlib.sha256(partial.read_bytes()).hexdigest()!=SHA:raise RuntimeError('Installer compiler checksum mismatch')
            os.replace(partial,package)
        finally:partial.unlink(missing_ok=True)
    destination=ROOT/'build/innosetup';compiler=destination/'ISCC.exe'
    if not compiler.is_file():
        startup=subprocess.STARTUPINFO();startup.dwFlags|=subprocess.STARTF_USESHOWWINDOW;startup.wShowWindow=0
        subprocess.run([str(package),'/PORTABLE=1','/VERYSILENT','/CURRENTUSER','/NOICONS','/NORESTART','/SUPPRESSMSGBOXES','/SP-',f'/DIR={destination}'],check=True,timeout=180,creationflags=0x08000000,startupinfo=startup)
    if not compiler.is_file():raise RuntimeError('Portable Inno compiler was not staged')
    manifest={'version':'7.1.1','url':URL,'sha256':SHA,'portable':True,'compiler':str(compiler)}
    (ROOT/'build/innosetup-provenance.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8');print(json.dumps(manifest))
if __name__=='__main__':main()
