"""Verify transparency without editing images: uv run --with pillow==13.0.0 python scripts/verify-icon-alpha.py [--bundle APP]."""
import argparse
import hashlib
import json
import plistlib
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
ICONS = ROOT / 'desktop/src-tauri/icons'
reports = []

def check(image, label):
    assert image.mode == 'RGBA', f'{label}: missing RGBA'
    alpha = image.getchannel('A')
    assert alpha.getextrema() == (0, 255), f'{label}: alpha must include 0 and 255'
    for point in [(0, 0), (image.width-1, 0), (0, image.height-1), (image.width-1, image.height-1)]:
        assert alpha.getpixel(point) == 0, f'{label}: opaque corner'
    assert alpha.getbbox(), f'{label}: empty image'
    # The empty right margin must remain transparent even at small sizes.
    assert alpha.getpixel((int(image.width * .98), int(image.height * .65))) == 0, f'{label}: background filled'
    reports.append({'image': label, 'size': image.size, 'alpha': alpha.getextrema()})

def inspect(path):
    with Image.open(path) as image:
        if image.format == 'ICO':
            for size in sorted(image.ico.sizes()):
                check(image.ico.getimage(size), f'{path.name}:{size}')
        elif image.format == 'ICNS':
            for size in image.info['sizes']:
                check(image.icns.getimage(size), f'{path.name}:{size}')
        else:
            check(image, path.name)

parser = argparse.ArgumentParser()
parser.add_argument('--bundle', type=Path)
args = parser.parse_args()
inspect(ROOT / 'assets/creatorx-icon/transparent.png')
for path in sorted(ICONS.glob('*.png')):
    inspect(path)
inspect(ICONS / 'icon.ico')
inspect(ICONS / 'icon.icns')
if args.bundle:
    contents = args.bundle / 'Contents'
    info = plistlib.loads((contents / 'Info.plist').read_bytes())
    name = info['CFBundleIconFile']
    if not name.endswith('.icns'):
        name += '.icns'
    bundled_icon = contents / 'Resources' / name
    assert bundled_icon.read_bytes() == (ICONS / 'icon.icns').read_bytes(), 'Bundle icon differs from validated ICNS'
    inspect(bundled_icon)
    executable = contents / 'MacOS' / info['CFBundleExecutable']
    assert (ICONS / 'icon.png').read_bytes() in executable.read_bytes(), 'Runtime PNG missing from executable'
    reports.append({'bundle': str(args.bundle), 'icon_sha256': hashlib.sha256(bundled_icon.read_bytes()).hexdigest(), 'runtime_png': 'embedded unchanged'})
print(json.dumps(reports, indent=2))
