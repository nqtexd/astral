"""Verify a portable Astral folder, every manifest hash, archive CRC and sidecar."""
import hashlib
import json
from pathlib import Path
import sys
import zipfile

package = Path(sys.argv[1]).resolve()
archive = package.with_name(package.name + '.zip')
required = ['Astral-Studio.exe', 'astral.exe', 'resources/ffmpeg.exe',
            'docs/ASTRAL-ASSISTANCE.md', 'models/pose-prior-v1.json',
            'models/pose-prior-v1-report.json', 'models/MODEL_CARD.md',
            'mcp-servers.json', 'COPYING', 'THIRD_PARTY/dependencies.json']
for name in required:
    assert (package / name).is_file(), name
entries = {}
for line in (package / 'SHA256SUMS.txt').read_text(encoding='utf-8-sig').splitlines():
    digest, name = line.split('  ', 1)
    target = (package / name).resolve()
    assert target.is_relative_to(package), name
    assert hashlib.file_digest(target.open('rb'), 'sha256').hexdigest() == digest, name
    entries[name] = digest
actual = {p.relative_to(package).as_posix() for p in package.rglob('*') if p.is_file() and p.name != 'SHA256SUMS.txt'}
assert set(entries) == actual
with zipfile.ZipFile(archive) as z:
    assert z.testzip() is None
    for name, digest in entries.items():
        assert hashlib.sha256(z.read(f'{package.name}/{name}')).hexdigest() == digest, name
expected = archive.with_name(archive.name + '.sha256').read_text(encoding='utf-8-sig').split()[0]
assert hashlib.file_digest(archive.open('rb'), 'sha256').hexdigest() == expected
config = json.loads((package / 'mcp-servers.json').read_text(encoding='utf-8-sig'))
assert set(config['mcpServers']) == {'astral-projects', 'astral-animation'}
assert json.loads((package / 'models/pose-prior-v1.json').read_text())['training_samples'] == 8192
print(json.dumps({'verified': True, 'files': len(entries), 'zip_crc': True,
                  'archive_sha256': expected, 'models_and_mcp_profiles': True}))
