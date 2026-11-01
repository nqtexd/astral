"""Publish reviewed local artifacts; credentials stay in Git Credential Manager."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import urllib.error
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = 'nqtexd/astral'

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--notes', type=Path, required=True)
    parser.add_argument('--asset', action='append', type=Path, default=[])
    parser.add_argument('--publish', action='store_true', help='Publish the prepared early-access draft')
    args = parser.parse_args()
    environment = {**os.environ, 'GIT_TERMINAL_PROMPT':'0', 'GCM_INTERACTIVE':'Never'}
    result = subprocess.run(['git','-c','credential.interactive=false','credential','fill'],
                            input='protocol=https\nhost=github.com\n\n', text=True,
                            capture_output=True, env=environment, cwd=ROOT)
    credentials = dict(line.split('=',1) for line in result.stdout.splitlines() if '=' in line)
    token = os.getenv('GH_TOKEN') or os.getenv('GITHUB_TOKEN') or credentials.get('password')
    if not token:
        raise SystemExit('Sign in with Git Credential Manager; do not put credentials in source files.')
    headers = {'Authorization':'Bearer '+token, 'Accept':'application/vnd.github+json',
               'User-Agent':'Astral-release', 'X-GitHub-Api-Version':'2022-11-28'}

    def api(method, path, value=None, data=None, content_type='application/json'):
        url = path if path.startswith('https://uploads.github.com/') else 'https://api.github.com'+path
        body = json.dumps(value).encode() if value is not None else data
        request = urllib.request.Request(url, method=method, data=body,
            headers={**headers, 'Content-Type':content_type})
        with urllib.request.urlopen(request, timeout=180) as response:
            raw = response.read()
            return json.loads(raw) if raw else None

    head = subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    tag = subprocess.check_output(['git','rev-parse',args.tag+'^{commit}'],cwd=ROOT,text=True).strip()
    remote = subprocess.check_output(['git','ls-remote','origin','refs/heads/main'],cwd=ROOT,text=True).split()[0]
    if tag != head or remote != head:
        raise SystemExit('Push the reviewed commit and matching tag before publishing.')
    prefix = '/repos/'+REPOSITORY
    notes = args.notes.read_text(encoding='utf-8')
    releases = api('GET',prefix+'/releases?per_page=100')
    release = next((r for r in releases if r['tag_name']==args.tag),None)
    payload = {'tag_name':args.tag, 'target_commitish':head,
               'name':'Astral '+args.tag.removeprefix('v')+' · Windows early access',
               'body':notes, 'draft':True, 'prerelease':True}
    if release is None:
        release = api('POST',prefix+'/releases',payload)
    elif release['draft']:
        release = api('PATCH',prefix+'/releases/'+str(release['id']),payload)
    else:
        raise SystemExit('This tag already has a published release; use a new version.')
    for file in args.asset:
        if not file.is_file():
            raise SystemExit('Missing release artifact: '+str(file))
        digest = 'sha256:'+hashlib.sha256(file.read_bytes()).hexdigest()
        existing = next((a for a in release['assets'] if a['name']==file.name),None)
        if existing and existing.get('digest')==digest:
            continue
        if existing:
            api('DELETE',prefix+'/releases/assets/'+str(existing['id']))
        upload = release['upload_url'].split('{',1)[0]+'?name='+urllib.parse.quote(file.name)
        asset = api('POST',upload,data=file.read_bytes(),content_type='application/octet-stream')
        print(json.dumps({'uploaded':asset['name'],'bytes':asset['size'],'sha256':digest}))
    api('PATCH',prefix,{'description':'AI-built native Rust animation studio: editable frames, rigs, storyboard, audio, VFX and MCP agent control. Windows early access.', 'homepage':'https://github.com/nqtexd/astral#readme'})
    api('PUT',prefix+'/topics',{'names':['animation','animation-studio','stickman','rust','egui','wgpu','mcp','frame-by-frame','ai-built','ai-assisted','offline','windows','open-source']})
    if args.publish:
        release = api('PATCH',prefix+'/releases/'+str(release['id']),{'draft':False,'prerelease':True,'make_latest':'false'})
    print(json.dumps({'release':release['html_url'],'draft':release['draft'],'prerelease':release['prerelease'],'commit':head}))

if __name__ == '__main__':
    main()
