"""Package a native macOS development build. No Developer ID/notarization claim."""
from pathlib import Path
import argparse,hashlib,json,os,platform,plistlib,shutil,subprocess,tarfile

ROOT=Path(__file__).resolve().parents[1]
from astral_notices import notices
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--include-encoder',action='store_true');args=parser.parse_args()
    if platform.system()!='Darwin':raise SystemExit('A native Mac build runner is required; packaging was not attempted.')
    import tomllib
    version=tomllib.loads((ROOT/'Cargo.toml').read_text())['workspace']['package']['version']
    arch=platform.machine();destination=ROOT/f'release/Astral-{version}-macos-{arch}'
    app=destination/'Astral Preview.app';contents=app/'Contents';binary=contents/'MacOS';resources=contents/'Resources'
    binary.mkdir(parents=True,exist_ok=True);resources.mkdir(exist_ok=True)
    for source,target in [('astral-studio','Astral'),('astral','astral')]:
        shutil.copy2(ROOT/'target/release'/source,binary/target)
    # The desktop encoder resolver also checks executable-parent/resources.
    encoder=binary/'resources';encoder.mkdir(exist_ok=True)
    if args.include_encoder:
        for source in (ROOT/'build/video-encoder').iterdir():
            if source.is_file():shutil.copy2(source,encoder/source.name)
    for directory in ['models','docs']:
        shutil.copytree(ROOT/directory,resources/directory,dirs_exist_ok=True)
    for directory in ['backend','codecs','src','vendor']:
        shutil.copytree(ROOT/directory,resources/directory,dirs_exist_ok=True,ignore=shutil.ignore_patterns('__pycache__','*.pyc'))
    (resources/'scripts').mkdir(exist_ok=True)
    for helper in ['astral-codec.py','astral-sticknodes.mjs','astral-decode-container.mjs','astral-evaluate-legacy.mjs','astral-source-input.mjs','astral-psd.mjs']:
        shutil.copy2(ROOT/'scripts'/helper,resources/'scripts'/helper)
    # Runtime resolution is relative to the executable. Keep sources in Resources
    # and use internal relative symlinks rather than duplicate the whole package.
    for directory in ['models','scripts','src','vendor','backend','codecs']:
        link=binary/directory
        if not link.exists():link.symlink_to(Path('../Resources')/directory,target_is_directory=True)
    for source in ['COPYING','README.md','THIRD_PARTY_NOTICES.md','AI_PROVENANCE.md','CREDITS.md']:shutil.copy2(ROOT/source,resources/source)
    for name,data in notices(ROOT).items():
        file=resources/'THIRD_PARTY'/name;file.parent.mkdir(parents=True,exist_ok=True);file.write_bytes(data)
    plist={'CFBundleExecutable':'Astral','CFBundleIdentifier':'org.astral.studio.preview','CFBundleName':'Astral Preview','CFBundleDisplayName':'Astral Preview','CFBundleShortVersionString':version,'CFBundleVersion':''.join(version.split('.'))+'0','NSMicrophoneUsageDescription':'Record a voice clip locally when you press Record in Astral.','CFBundlePackageType':'APPL','LSMinimumSystemVersion':'13.0','NSHighResolutionCapable':True}
    (contents/'Info.plist').write_bytes(plistlib.dumps(plist))
    subprocess.run(['codesign','--force','--deep','--sign','-',str(app)],check=True)
    # Ad-hoc signing supports local testing; Gatekeeper acceptance is not claimed.
    subprocess.run(['codesign','--verify','--deep','--strict',str(app)],check=True)
    archive=destination.with_name(destination.name+'.tar.gz')
    with tarfile.open(archive,'w:gz')as tar:tar.add(destination,arcname=destination.name)
    report={'status':'development-preview','architecture':arch,'signature':'ad-hoc only','notarized':False,'feature_parity':False,'runtime_prerequisites':['Node.js 24+ for PSD/modern legacy imports','Rhubarb macOS runtime via ASTRAL_RHUBARB for recognized lip sync'],'archive':archive.name,'sha256':hashlib.sha256(archive.read_bytes()).hexdigest()}
    (destination/'build-report.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
