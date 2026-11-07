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

def request(route, raw=False):
    env = {**os.environ, 'GIT_TERMINAL_PROMPT':'0', 'GCM_INTERACTIVE':'Never'}
    result = subprocess.run(['git','credential','fill'],input='protocol=https\nhost=github.com\n\n',text=True,capture_output=True,env=env,check=True)
    credential = dict(line.split('=',1) for line in result.stdout.splitlines() if '=' in line)
    token = credential.get('password')
    if not token:
        raise RuntimeError('GitHub credential unavailable; sign in using the normal Git credential flow.')
    url='https://api.github.com/repos/'+REPO+'/'+route
    req=urllib.request.Request(url,headers={'Authorization':'Bearer '+token,'Accept':'application/vnd.github+json','User-Agent':'altrun-build-operator','X-GitHub-Api-Version':'2022-11-28'})
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

def main():
    p=argparse.ArgumentParser()
    p.add_argument('action',choices=['runs','jobs','logs','download','artifacts'])
    p.add_argument('--sha');p.add_argument('--id',type=int);p.add_argument('--output',type=Path)
    a=p.parse_args()
    if a.action=='runs':
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
