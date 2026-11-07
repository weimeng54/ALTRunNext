"""Read this repository's Actions using existing Git Credential Manager auth.

Credentials stay in memory and are never printed, written, or forwarded to
artifact redirect hosts. No dependency installation is needed.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import urllib.request
import urllib.parse
import urllib.error
import base64
import datetime
import re
import hashlib

REPO = 'weimeng54/altrun'

class SafeRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        new = super().redirect_request(req, fp, code, msg, headers, newurl)
        if new and urllib.parse.urlparse(newurl).hostname != 'api.github.com':
            new.remove_header('Authorization')
        return new

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None

def request(route, raw=False, data=None, method=None):
    env = {**os.environ, 'GIT_TERMINAL_PROMPT':'0', 'GCM_INTERACTIVE':'Never'}
    result = subprocess.run(['git','credential','fill'],input='protocol=https\nhost=github.com\n\n',text=True,capture_output=True,env=env,check=True)
    credential = dict(line.split('=',1) for line in result.stdout.splitlines() if '=' in line)
    token = credential.get('password')
    if not token:
        raise RuntimeError('GitHub credential unavailable; sign in using the normal Git credential flow.')
    url='https://api.github.com/repos/'+REPO+'/'+route
    req=urllib.request.Request(url,data=None if data is None else json.dumps(data).encode(),method=method,headers={'Authorization':'Bearer '+token,'Accept':'application/vnd.github+json','Content-Type':'application/json','User-Agent':'altrun-build-operator','X-GitHub-Api-Version':'2022-11-28'})
    try:
        with urllib.request.build_opener(NoRedirect() if raw else SafeRedirect()).open(req,timeout=30) as response:
            body=response.read()
    except urllib.error.HTTPError as error:
        if not raw or error.code != 302:
            raise
        location=error.headers['Location']
        parsed=urllib.parse.urlparse(location)
        if parsed.scheme!='https' or not any((parsed.hostname or '').endswith(s) for s in ('.githubusercontent.com','.blob.core.windows.net','.github.com')):
            raise RuntimeError('Unexpected artifact download host') from None
        # No GitHub credential is sent to this signed, temporary download URL.
        result=subprocess.run(['curl.exe','--fail','--silent','--show-error','--max-time','40',location],capture_output=True,check=True)
        body=result.stdout
    return body if raw else json.loads(body)

def sync():
    """Fallback for unreliable Git HTTPS: reproduce the exact local Git objects.

    Ref update is non-forced and only happens after tree AND commit hashes match.
    Never uploads the working directory or ignored private measurement files.
    """
    def git(*args):
        return subprocess.check_output(['git',*args])
    head=git('rev-parse','HEAD').decode().strip()
    remote=request('git/ref/heads/codex/performance-spike')['object']['sha']
    if remote==head:
        print(json.dumps({'synced':head,'transport':'already-up-to-date'}));return
    parent=git('rev-parse','HEAD^').decode().strip()
    if parent!=remote:
        raise RuntimeError('API fallback only accepts one commit directly on remote HEAD; no history rewrite.')
    changes=git('diff-tree','--no-commit-id','--name-status','-r','HEAD').decode().splitlines()
    items=[]
    for change in changes:
        state,name=change.split('\t',1)
        if state=='D':
            items.append({'path':name,'mode':'100644','type':'blob','sha':None});continue
        content=git('show','HEAD:'+name)
        blob=request('git/blobs',data={'content':base64.b64encode(content).decode(),'encoding':'base64'})
        expected=git('rev-parse','HEAD:'+name).decode().strip()
        if blob['sha']!=expected:
            raise RuntimeError('Remote blob hash mismatch')
        items.append({'path':name,'mode':'100644','type':'blob','sha':blob['sha']})
    base=request('git/commits/'+remote)['tree']['sha']
    tree=request('git/trees',data={'base_tree':base,'tree':items})['sha']
    if tree!=git('rev-parse','HEAD^{tree}').decode().strip():
        raise RuntimeError('Remote tree hash mismatch; branch was not changed')
    headers,message=git('cat-file','commit','HEAD').decode().split('\n\n',1)
    metadata={}
    for line in headers.splitlines():
        for key in ['author','committer']:
            if line.startswith(key+' '):
                match=re.fullmatch(r'.*? (.*?) <(.*?)> (\d+) ([+-]\d{4})',line)
                name,email,stamp,offset=match.groups()
                minutes=(int(offset[1:3])*60+int(offset[3:]))*(1 if offset[0]=='+' else -1)
                date=datetime.datetime.fromtimestamp(int(stamp),datetime.timezone(datetime.timedelta(minutes=minutes))).isoformat(timespec='seconds')
                metadata[key]={'name':name,'email':email,'date':date}
    commit=request('git/commits',data={'message':message.rstrip('\n'),'tree':tree,'parents':[parent],**metadata})['sha']
    normalized=None
    if commit!=head:
        # GitHub's commit API canonicalizes date offsets to UTC. Permit only
        # that metadata-only transformation, never a tree/message/parent change.
        for author_utc in (False,True):
            for committer_utc in (False,True):
                h=headers
                for key,convert in [('author',author_utc),('committer',committer_utc)]:
                    if convert:h=re.sub(r'^('+key+r') (.*) (\d+) [+-]\d{4}$',r'\1 \2 \3 +0000',h,flags=re.MULTILINE)
                for trailing in ('','\n','\n\n'):
                    raw=(h+'\n\n'+message.rstrip('\n')+trailing).encode()
                    expected=hashlib.sha1(b'commit '+str(len(raw)).encode()+b'\0'+raw).hexdigest()
                    if expected==commit:normalized=raw
        if normalized is None:
            raise RuntimeError('Remote commit hash mismatch beyond date/message terminator metadata; branch was not changed')
        written=subprocess.check_output(['git','hash-object','-t','commit','-w','--stdin'],input=normalized).decode().strip()
        if written!=commit:raise RuntimeError('Local canonical commit mismatch')
    request('git/refs/heads/codex/performance-spike',data={'sha':commit,'force':False},method='PATCH')
    if normalized is not None:
        subprocess.run(['git','update-ref','refs/heads/codex/performance-spike',commit,head],check=True)
    subprocess.run(['git','update-ref','refs/remotes/github/codex/performance-spike',commit],check=True)
    print(json.dumps({'synced':commit,'transport':'GitHub Git Database API','changed_files':len(items),'utc_metadata_normalized':normalized is not None}))

def main():
    p=argparse.ArgumentParser()
    p.add_argument('action',choices=['runs','jobs','logs','download','artifacts','sync'])
    p.add_argument('--sha');p.add_argument('--id',type=int);p.add_argument('--output',type=Path)
    a=p.parse_args()
    if a.action=='sync':
        sync()
    elif a.action=='runs':
        result=request('actions/runs?per_page=10'+('&head_sha='+a.sha if a.sha else ''))
        print(json.dumps([{k:r.get(k) for k in ['id','head_sha','status','conclusion','html_url','created_at']} for r in result['workflow_runs']],indent=2))
    elif a.action=='jobs':
        result=request(f'actions/runs/{a.id}/jobs')
        print(json.dumps([{k:r.get(k) for k in ['id','name','status','conclusion','steps']} for r in result['jobs']],indent=2))
    elif a.action=='artifacts':
        result=request(f'actions/runs/{a.id}/artifacts')
        print(json.dumps([{k:r.get(k) for k in ['id','name','size_in_bytes','expired']} for r in result['artifacts']],indent=2))
    else:
        if not a.id or not a.output:
            p.error('--id and --output are required')
        route=f'actions/artifacts/{a.id}/zip' if a.action=='download' else f'actions/jobs/{a.id}/logs'
        data=request(route,raw=True)
        a.output.parent.mkdir(parents=True,exist_ok=True)
        a.output.write_bytes(data)
        print(json.dumps({'saved':str(a.output),'bytes':len(data)}))

if __name__=='__main__':
    main()
