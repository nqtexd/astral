"""Collect Cargo dependency and embedded-font notices for native packages."""
from pathlib import Path
import json,re,subprocess

def notices(root,target=None):
    if target is None:
        version=subprocess.check_output(['rustc','-vV'],encoding='utf-8')
        target=next(line.split(': ',1)[1]for line in version.splitlines()if line.startswith('host: '))
    metadata=json.loads(subprocess.check_output(['cargo','metadata','--format-version','1','--locked','--filter-platform',target],cwd=root,encoding='utf-8'))
    output={}
    dependencies=[]
    for package in metadata['packages']:
        dependencies.append({k:package.get(k)for k in ['name','version','license','source']})
        directory=Path(package['manifest_path']).parent
        # License files may live beside embedded font files, not only at root.
        for file in directory.rglob('*'):
            if file.is_file()and re.match(r'^(LICENSE|COPYING|NOTICE|OFL)(\b|[._-])',file.name,re.I):
                if file.stat().st_size>2*1024*1024:continue
                output[f'{package["name"]}-{package["version"]}/{file.relative_to(directory).as_posix()}']=file.read_bytes()
    for file in (root/'vendor/text').glob('*'):
        if file.is_file()and any(v in file.name.lower()for v in ['license','ofl']):output['embedded-fonts/'+file.name]=file.read_bytes()
    output['dependencies.json']=json.dumps(dependencies,indent=2).encode()
    return output
