"""Package the native preview sources without build caches or user productions."""
from pathlib import Path
import hashlib
import json
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]

def main():
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['workspace']['package']['version']
    output = ROOT / 'release' / 'preview' / f'Astral-{version}-source.zip'
    output.parent.mkdir(parents=True, exist_ok=True)
    files = []
    for name in ['Cargo.toml', 'Cargo.lock', 'COPYING', 'README.md', '.gitignore', 'AI_PROVENANCE.md', 'CREDITS.md', 'THIRD_PARTY_NOTICES.md', 'CONTRIBUTING.md', 'mcp.example.json']:
        files.append(ROOT / name)
    for directory in ['crates', 'platforms', 'scripts', 'docs', 'assets', 'models', 'vendor/text', 'codecs', 'backend', '.github']:
        for path in (ROOT / directory).rglob('*'):
            relative = path.relative_to(ROOT)
            if not path.is_file() or path.is_symlink():
                continue
            if any(part in {'__pycache__', 'target', 'node_modules', '.git'} or part.endswith('.xcodeproj') for part in relative.parts):
                continue
            if path.suffix.lower() in {'.pyc', '.keystore', '.p12', '.mobileprovision'} or path.name == 'Generated-Info.plist':
                continue
            files.append(path)
    inventory = {str(p.relative_to(ROOT)).replace('\\', '/'): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(files))}
    with zipfile.ZipFile(output, 'w', zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name in inventory:
            archive.write(ROOT / name, name)
        archive.writestr('SOURCE-MANIFEST.json', json.dumps(inventory, indent=2))
    print(json.dumps({'artifact':str(output.relative_to(ROOT)), 'files':len(inventory), 'bytes':output.stat().st_size, 'sha256':hashlib.sha256(output.read_bytes()).hexdigest()}, indent=2))

if __name__ == '__main__':
    main()
