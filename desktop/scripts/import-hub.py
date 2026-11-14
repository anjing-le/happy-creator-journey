"""Migrate a private Hub API export through CreatorX CLI; never deletes Hub data.
python import-hub.py --client /path/to/creatorx --snapshot /private/hub-export
Snapshot must contain unmodified items.json and categories.json API responses.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import urllib.parse
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument('--client', type=Path, required=True)
parser.add_argument('--snapshot', type=Path, required=True)
parser.add_argument('--source', default='https://hub.anjing.cc')
args = parser.parse_args()
client = str(args.client.resolve())
snapshot = args.snapshot.resolve()

def cli(*arguments, data=None):
    result = subprocess.run([client, *arguments], input=json.dumps(data, ensure_ascii=False).encode() if data is not None else None, capture_output=True, timeout=45)
    stream = result.stdout if result.returncode == 0 else result.stderr
    try: payload = json.loads(stream)
    except Exception: raise RuntimeError('CreatorX CLI returned no valid JSON; update the client first')
    if result.returncode or not payload.get('ok'): raise RuntimeError(payload.get('error', 'CLI failed'))
    return payload['data']

schema = cli('schema')
assert schema['name'] == 'CreatorX CLI'
raw = json.loads((snapshot / 'items.json').read_text())
rows = raw if isinstance(raw, list) else raw['items']
raw_categories = json.loads((snapshot / 'categories.json').read_text())
categories = raw_categories if isinstance(raw_categories, list) else raw_categories['categories']
category_names = {category['id']: category['name'] for category in categories}
assert len({row['id'] for row in rows}) == len(rows), 'Duplicate Hub IDs'
for category in categories: cli('library','add-category','--name',category['name'])
existing = {entry['id']: entry for entry in cli('library', 'list')['entries']}
image_directory = snapshot / 'uploads'
image_directory.mkdir(exist_ok=True)
image_directory.chmod(0o700)
image_cache = {}

def import_image(url):
    if url in image_cache: return image_cache[url]
    address = urllib.parse.urljoin(args.source, url)
    origin = urllib.parse.urlparse(args.source)
    parsed = urllib.parse.urlparse(address)
    if parsed.scheme != 'https' or parsed.netloc != origin.netloc or not parsed.path.startswith('/uploads/'):
        raise RuntimeError('Image must be an HTTPS Hub upload')
    suffix = Path(parsed.path).suffix.lower()
    target = image_directory / (hashlib.sha256(address.encode()).hexdigest() + suffix)
    if not target.exists():
        request = urllib.request.Request(address, headers={'User-Agent':'CreatorX-Migration/0.0.7'})
        with urllib.request.urlopen(request, timeout=30) as response:
            if urllib.parse.urlparse(response.geturl()).netloc != origin.netloc: raise RuntimeError('Unexpected image redirect')
            content = response.read(12 * 1024 * 1024 + 1)
        if len(content) > 12 * 1024 * 1024: raise RuntimeError('Reference image exceeds 12 MB')
        temporary = target.with_suffix(target.suffix + '.tmp')
        temporary.write_bytes(content); temporary.chmod(0o600); temporary.replace(target)
    content = target.read_bytes()
    result = cli('library', 'import-image', '--file', str(target))
    name = result['name']
    assert name.split('.')[0] == hashlib.sha256(content).hexdigest(), 'Image byte hash mismatch'
    image_cache[url] = name
    return name

report = {'source': args.source, 'sourceCount':len(rows), 'imported':0, 'unchanged':0, 'conflicts':[], 'errors':[], 'items':[]}
for row in rows:
    target_id = 'hub-' + row['id']
    category = category_names.get(row.get('category_id') or 'default')
    if category is None: raise RuntimeError('Hub category is missing from export')
    try:
        images = [import_image(row['image_url'])] if row.get('image_url') else []
        entry = {'schemaVersion':2,'id':target_id,'kind':'prompt','name':row['desc'],'prompt':row['content'],'category':category,'tags':[], 'notes':'', 'images':images,'source':'Anjing Hub · '+row['id'],'originCreatedAt':row.get('created_at'),'originUpdatedAt':row.get('updated_at')}
        if target_id in existing:
            if any(existing[target_id].get(key) != entry.get(key) for key in ['kind','name','prompt','category','images','source']):
                report['conflicts'].append({'id':target_id,'name':row['desc']}); continue
            saved = existing[target_id]; report['unchanged'] += 1
        else:
            saved = cli('library','save','--stdin','--if-revision','0',data=entry); report['imported'] += 1
        digest = hashlib.sha256(row['content'].encode()).hexdigest()
        assert hashlib.sha256(saved['prompt'].encode()).hexdigest() == digest, 'Prompt byte hash mismatch'
        assert saved['name'] == row['desc'] and saved['category'] == category
        report['items'].append({'sourceId':row['id'],'id':saved['id'],'name':saved['name'],'category':category,'contentSHA256':digest,'images':images})
    except Exception as error: report['errors'].append({'id':target_id,'error':str(error)})
report['imageReferences'] = sum(len(row['images']) for row in report['items'])
report['uniqueImages'] = len(set(image_cache.values()))
output = snapshot / 'migration-report.json'
output.write_text(json.dumps(report,ensure_ascii=False,indent=2)); output.chmod(0o600)
print(json.dumps({key:value for key,value in report.items() if key != 'items'},ensure_ascii=False))
raise SystemExit(1 if report['errors'] or report['conflicts'] else 0)
