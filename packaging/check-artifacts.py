#!/usr/bin/env python3
"""Record bundle checksums and the actual AppImage glibc requirement in dist/build-info.json."""
import hashlib
import json
import re
import subprocess
from build import DIST, WORK

record = json.loads((DIST / 'build-info.json').read_text())
versions = set()
for folder in ['bin', 'lib', 'libexec']:
    for path in (WORK / 'Toniator.AppDir/usr' / folder).rglob('*'):
        if not path.is_file():
            continue
        with path.open('rb') as stream:
            if stream.read(4) != b'\x7fELF':
                continue
        symbols = subprocess.check_output(['objdump', '-T', str(path)], text=True)
        versions.update(re.findall(r'GLIBC_(\d+(?:\.\d+)+)', symbols))
if versions:
    record['appimage_minimum_glibc'] = max(versions, key=lambda v: tuple(map(int, v.split('.'))))
record['artifacts'] = []
lines = []
for suffix in ['AppImage', 'flatpak']:
    path = DIST / ('Toniator-0.2.0-x86_64.' + suffix)
    if not path.exists():
        continue
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    record['artifacts'].append({'name': path.name, 'bytes': path.stat().st_size, 'sha256': digest})
    lines.append(digest + '  ' + path.name)
(DIST / 'build-info.json').write_text(json.dumps(record, indent=2) + '\n')
(DIST / 'SHA256SUMS').write_text('\n'.join(lines) + '\n')
print(json.dumps({'minimum_glibc': record.get('appimage_minimum_glibc'), 'artifacts': record['artifacts']}, indent=2))
