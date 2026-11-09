#!/usr/bin/env python3
"""Reproduce/verify notices from the locked, locally unpacked Cargo sources."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version', '1'], cwd=root))
texts = {}
sections = ['# Third-party notices\n\nGenerated from Cargo.lock and cargo metadata. Includes all locked targets and build dependencies (a conservative superset of linked code). License texts below are copied verbatim from the unpacked crate sources; SHA-256 identifies each original file. Regenerate or verify with `python3 scripts/third-party-notices.py [--check]`.\n']
for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
    if package['name'] == 'clayui':
        continue
    folder = Path(package['manifest_path']).parent
    files = sorted(f for f in folder.rglob('*') if f.is_file() and (f.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE')) or f.name == 'COPYRIGHT'))
    if not files:
        raise SystemExit('Missing real license files: ' + package['name'])
    sections.append(f"\n## {package['name']} {package['version']}\n\nDeclared license: `{package['license']}`.\n")
    for file in files:
        raw = file.read_bytes()
        checksum = hashlib.sha256(raw).hexdigest()
        label = 'license-' + checksum[:16]
        sections.append(f"\n- `{file.relative_to(folder)}` (SHA-256 `{checksum}`): [full text](#{label}).\n")
        texts.setdefault(checksum, (label, raw.decode('utf-8')))
    # ring's ISC file points at per-source copyright notices. Preserve those too.
    if package['name'] == 'ring':
        copyrights = set()
        for file in folder.rglob('*'):
            if file.is_file() and file.suffix in ('.c', '.h', '.rs', '.pl', '.S', '.asm'):
                for line in file.read_text(errors='replace').splitlines():
                    if 'copyright' in line.lower() and len(line) < 500:
                        copyrights.add(line.strip(' /*#;'))
        sections.append('\nSource copyright notices:\n\n```text\n' + '\n'.join(sorted(copyrights)) + '\n```\n')
for _, (label, text) in texts.items():
    sections.append(f'\n<a id="{label}"></a>\n\n### {label}\n\n```text\n{text}' + ('' if text.endswith('\n') else '\n') + '```\n')
result = ''.join(sections)
output = root / 'THIRD_PARTY_NOTICES.md'
if '--check' in sys.argv:
    if not output.exists() or output.read_text() != result:
        raise SystemExit('Notices differ from locked crate license files')
    print('PASS: every notice matches the locked crate license files')
else:
    output.write_text(result)
    print('Wrote THIRD_PARTY_NOTICES.md from locked crate license files')
