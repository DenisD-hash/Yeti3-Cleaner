#!/bin/bash
set -euo pipefail
TASK_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$TASK_ROOT"
VERSION="$(awk -F'"' '/^version = / {print $2; exit}' Cargo.toml)"
ARCH="${1:-universal}"
bash scripts/build-app.sh "$ARCH"
STAGING="$(mktemp -d "${TMPDIR:-/tmp}/yeti3-dmg.XXXXXX")"
trap 'rm -rf "$STAGING"' EXIT
cp -R dist/$ARCH/Yeti3-Cleaner.app "$STAGING/"
ln -s /Applications "$STAGING/Applications"
cp docs/INSTALL-RU.txt "$STAGING/Установка.txt"
mkdir -p downloads
hdiutil create -volname "YETI3 Cleaner $VERSION" -srcfolder "$STAGING" -ov -format UDZO "downloads/Yeti3-Cleaner-$VERSION-$ARCH.dmg"
ditto -c -k --sequesterRsrc --keepParent dist/$ARCH/Yeti3-Cleaner.app "downloads/Yeti3-Cleaner-$VERSION-$ARCH.zip"
python3 - "$VERSION" "$ARCH" <<'PY'
import hashlib, json, sys
from pathlib import Path
v, arch = sys.argv[1:]
p = Path('downloads')
files = [p / f'Yeti3-Cleaner-{v}-{arch}.{ext}' for ext in ('dmg', 'zip')]
hashes = {f.name: hashlib.sha256(f.read_bytes()).hexdigest() for f in files}
(p / ('SHA256SUMS-' + arch + '.txt')).write_text(''.join(f'{value}  {name}\n' for name, value in hashes.items()))
(p / ('latest-' + arch + '.json')).write_text(json.dumps({'version': v, 'prerelease': True, 'architecture': arch, 'minimum_macos': '14.0', 'url': f'https://raw.githubusercontent.com/lodos/Yeti3-Cleaner/master/downloads/{files[0].name}', 'sha256': hashes[files[0].name]}, indent=2) + '\n')
if arch == 'universal':
    (p / 'latest-prerelease.json').write_bytes((p / ('latest-' + arch + '.json')).read_bytes())
PY
hdiutil verify "downloads/Yeti3-Cleaner-$VERSION-$ARCH.dmg"
