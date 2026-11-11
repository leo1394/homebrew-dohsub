#!/usr/bin/env python3
"""Isolated CLI, daemon, installer and upgrade checks; uses synthetic credentials only."""
import base64
import hashlib
import http.client
import json
import os
from pathlib import Path
import pty
import re
import select
import shlex
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get('DOHSUB_TEST_BINARY', ROOT / 'target/debug/dohsub')).resolve()
VERSION = (ROOT / 'VERSION.txt').read_text().strip()
REPO = 'https://github.com/leo1394/homebrew-dohsub'
# Synthetic public A answer; tests never connect to the advertised proxy address.
PUBLIC_TEST_IPV4 = '93.184.216.34'
MOCK = r'''#!/usr/bin/env python3
import base64,json,os,socket,struct,sys,urllib.parse
from pathlib import Path
args=sys.argv[1:]
if '--config' in args:
    if os.environ.get('TEST_DOH_FAIL'): sys.exit(22)
    url=sys.stdin.read().split('"')[1]
    wire=base64.urlsafe_b64decode(urllib.parse.parse_qs(urllib.parse.urlparse(url).query)['dns'][0]+'===')
    i=12; labels=[]
    while wire[i]: n=wire[i];labels.append(wire[i+1:i+1+n].decode());i+=1+n
    host='.'.join(labels)
    if host.startswith('flaky.') and Path(os.environ['TEST_FAIL_FLAG']).exists():sys.exit(22)
    with open(os.environ['TEST_DNS_LOG'],'a') as f:f.write(host+'\n')
    question=wire[12:]
    negative=host.startswith('missing.')
    header=struct.pack('!6H',0,0x8183 if negative else 0x8180,1,0 if negative else 1,0,0)
    answer=b'' if negative else b'\xc0\x0c'+struct.pack('!HHIH',1,1,3600,4)+socket.inet_aton(os.environ['TEST_PUBLIC_IPV4'])
    sys.stdout.buffer.write(header+question+answer);sys.exit()
url=args[-1]
if os.environ.get('TEST_RELEASE_FAIL'): sys.exit(22)
if url.endswith('/releases/latest'):
    tag=os.environ.get('TEST_TAG','v1.0.0'); name='dohsub-'+os.environ.get('TEST_PLATFORM','aarch64-apple-darwin')
    obj={'tag_name':tag,'draft':False,'prerelease':bool(os.environ.get('TEST_PRERELEASE')),'assets':[{'name':n,'browser_download_url':'https://github.com/leo1394/homebrew-dohsub/releases/download/'+tag+'/'+n} for n in [name,'SHA256SUMS','install.sh']]}
    data=json.dumps(obj,indent=2).encode()
else:
    name=url.rsplit('/',1)[1]
    if name=='SHA256SUMS':data=Path(os.environ['TEST_SUMS']).read_bytes()
    elif name=='install.sh':data=Path(os.environ['TEST_INSTALLER']).read_bytes()
    elif name.startswith('dohsub-'):data=Path(os.environ['TEST_ASSET']).read_bytes()
    else:sys.exit(22)
if '--output' in args:Path(args[args.index('--output')+1]).write_bytes(data)
else:sys.stdout.buffer.write(data)
'''

def write(path, data, mode=0o600):
    path.write_text(data)
    path.chmod(mode)


def main():
    checks = 0
    with tempfile.TemporaryDirectory(prefix='ds-', dir='/private/tmp' if sys.platform == 'darwin' else '/tmp') as tmp:
        tmp = Path(tmp)
        fakebin = tmp / 'bin'; fakebin.mkdir()
        write(fakebin / 'curl', MOCK, 0o755)
        clip = tmp / 'clipboard'
        for name in ('pbcopy', 'pbpaste'):
            body = 'import os,sys\nfrom pathlib import Path\np=Path(os.environ["TEST_CLIP"])\n'
            body += 'p.write_bytes(sys.stdin.buffer.read())\n' if name == 'pbcopy' else 'sys.stdout.buffer.write(p.read_bytes())\n'
            write(fakebin / name, '#!/usr/bin/env python3\n' + body, 0o755)
        env = dict(os.environ, DOHSUB_HOME=str(tmp / 'state'), PATH=str(fakebin) + ':' + os.environ['PATH'], TEST_DNS_LOG=str(tmp / 'dns.log'), TEST_CLIP=str(clip), TEST_PUBLIC_IPV4=PUBLIC_TEST_IPV4, TEST_FAIL_FLAG=str(tmp/'fail-dns'), NO_COLOR='1')
        if sys.platform != 'darwin':
            write(fakebin / 'wl-copy', (fakebin / 'pbcopy').read_text(), 0o755)
            write(fakebin / 'wl-paste', (fakebin / 'pbpaste').read_text(), 0o755)
        def run(*args, code=0, binary=BINARY, extra=None):
            r = subprocess.run([str(binary), *map(str,args)], env=env | (extra or {}), capture_output=True, timeout=25)
            assert r.returncode == code, (args, r.returncode, r.stdout.decode(), r.stderr.decode())
            return r
        def check(value):
            nonlocal checks
            assert value
            checks += 1
        def status():
            s = socket.socket(socket.AF_UNIX); s.settimeout(3)
            s.connect(str(tmp / 'state/control.sock')); s.sendall(b'"status"\n')
            data = b''
            while b'\n' not in data:
                chunk=s.recv(65536)
                if not chunk:break
                data+=chunk
            s.close()
            return json.loads(data)
        def wait_for(predicate):
            end = time.monotonic() + 6
            while time.monotonic() < end:
                if predicate():return
                time.sleep(.05)
            raise AssertionError('Timed out waiting for isolated daemon state')
        def http_get(port, path, host=None, origin=False):
            c=http.client.HTTPConnection('127.0.0.1',port,timeout=3)
            headers={'Host':host or f'127.0.0.1:{port}'}
            if origin:headers['Origin']='https://other.example'
            c.request('GET',path,headers=headers);r=c.getresponse();out=(r.status,r.read());c.close();return out
        def interactive(args, answer, binary=BINARY):
            master, slave = pty.openpty()
            proc=subprocess.Popen([str(binary),*args],stdin=slave,stdout=slave,stderr=slave,env=env,close_fds=True)
            os.close(slave);data=b'';deadline=time.monotonic()+15;sent=False
            while proc.poll() is None and time.monotonic()<deadline:
                if select.select([master],[],[],.1)[0]:
                    try:data+=os.read(master,65536)
                    except OSError:break
                    if b'[y/N]' in data and not sent:
                        os.write(master,answer);sent=True
            if proc.poll() is None:proc.kill()
            rc=proc.wait();os.close(master)
            return rc,data,sent
        try:
            v=run('--version').stdout
            check(v==run('version').stdout and len(v.splitlines())==2 and f'dohsub version {VERSION} ('.encode() in v and REPO.encode() in v)
            for args in [[],['help'],['help','config','set'],['config','set','--help'],['upgrade','--help'],['status','--help'],['completion','zsh'],['man']]:run(*args)
            fresh=run('status').stdout
            check(b'Service: stopped' in fresh and b'Saved DoH: not configured' in fresh and b'Staged nodes: 0' in fresh)
            check(not (tmp/'state').exists())
            check(b'version' in run('versoin',code=2).stderr)
            check(b'--file' in run('import','--fiel','file',code=2).stderr)
            run('start',code=1)
            check(not (tmp/'state/control.sock').exists())
            # Stale sockets are stopped; invalid replies are not reported as stopped.
            control=tmp/'state/control.sock'
            stale=socket.socket(socket.AF_UNIX);stale.bind(str(control));stale.close()
            check(b'Service: stopped' in run('status').stdout)
            control.unlink()
            broken=socket.socket(socket.AF_UNIX);broken.bind(str(control));broken.listen(1)
            def invalid_response():
                connection,_=broken.accept()
                with connection:
                    connection.recv(4096);connection.sendall(b'invalid status\n')
            worker=threading.Thread(target=invalid_response,daemon=True);worker.start()
            failed=run('status',code=1)
            worker.join(timeout=3);broken.close();control.unlink()
            check(b'Service: unresponsive' in failed.stdout and b'Service: stopped' not in failed.stdout)
            run('config','set','--doh','http://example.com',code=1)
            run('config','set','--doh','https://dns.example/query?private=test')
            check(b'REDACTED' in run('config','--list').stdout and b'private=test' not in run('config','--list').stdout)
            nodefile=tmp/'nodes with spaces.txt'
            line='shadowsocks=one.example.com:8443, method=chacha20-ietf-poly1305, password=fixture-only, tag=US1'
            write(nodefile,line+'\nmalformed sensitive-fixture\n'+line)
            result=run('import','--file',nodefile,code=1)
            check('✓ US1: added' in result.stdout.decode() and result.stdout.decode().count('✗')==2 and b'sensitive-fixture' not in result.stdout+result.stderr)
            check(len(json.loads((tmp/'state/nodes.json').read_text()))==1)
            # Inputs are private, and malformed Unicode must never panic.
            public=tmp/'public';write(public,line,0o644);run('import','--file',public,code=1)
            write(nodefile,'ss://YWVzLTEyOC1nY206Zml4dHVyZQ@example.com:8443#%你')
            check(b'Invalid percent encoding' in run('import','--file',nodefile,code=1).stdout)
            run('start',code=1,extra={'TEST_DOH_FAIL':'1'})
            check(not (tmp/'state/control.sock').exists())
            occupied=socket.socket();occupied.bind(('127.0.0.1',0));port=occupied.getsockname()[1];occupied.listen()
            run('start','--port',port,code=1)
            run('config','set','--port',port)
            run('start')
            active=status();actual=active['port']
            check(actual>port)
            occupied.close()
            wait_for(lambda:status()['published_node_count']==1)
            token=(tmp/'state/token').read_text();path=f'/sub/{token}/nodes.conf'
            before=(tmp/'dns.log').read_bytes()
            current=run('status').stdout
            check(b'Service: running' in current and b'Health: ready' in current and f'PID: {active["pid"]}'.encode() in current and f'Listening: http://127.0.0.1:{actual}'.encode() in current)
            check(b'Active nodes: 1' in current and b'Published nodes: 1' in current and b'Saved DoH: configured' in current)
            check(token.encode() not in current and b'fixture-only' not in current and b'https://dns.example' not in current and b'private=test' not in current)
            check((tmp/'dns.log').read_bytes()==before)
            code,body=http_get(actual,path)
            check(code==200 and (PUBLIC_TEST_IPV4+':8443').encode() in body and b'fixture-only' in body)
            check(http_get(actual,'/sub/wrong/nodes.conf')[0]==404 and http_get(actual,path,host='attacker.example')[0]==403 and http_get(actual,path,origin=True)[0]==403)
            check(b'fixture-only' not in http_get(actual,'/health')[1] and token.encode() not in run('list').stdout)
            before=(tmp/'dns.log').read_text().splitlines()
            for _ in range(20):http_get(actual,path)
            check((tmp/'dns.log').read_text().splitlines()==before)
            write(nodefile,line+'\n'+line.replace('one.example.com','missing.example.com').replace('US1','TW1'))
            run('import','--file',nodefile)
            check(status()['node_count']==1)
            current=run('status').stdout
            check(b'Active nodes: 1' in current and b'Staged nodes: 2' in current)
            run('refresh');wait_for(lambda:status()['node_count']==2 and 'missing.example.com' in status()['dns_retries'])
            current=run('status').stdout
            check(b'Health: partial' in current and b'Active nodes: 2' in current and b'Published nodes: 1' in current and b'DNS retry hosts: 1' in current)
            calls=(tmp/'dns.log').read_text().splitlines().count('missing.example.com')
            check(720<=status()['dns_retries']['missing.example.com']['retry_in_seconds']<=900)
            http_get(actual,path);time.sleep(.1)
            check((tmp/'dns.log').read_text().splitlines().count('missing.example.com')==calls)
            run('refresh');wait_for(lambda:(tmp/'dns.log').read_text().splitlines().count('missing.example.com')>calls)
            check(status()['node_count']==2 and status()['published_node_count']==1)
            write(nodefile,line.replace('US1','SG1'))
            rc,_,prompted=interactive(['import','--file',str(nodefile)],b'n\n')
            check(rc==0 and prompted and status()['node_count']==2)
            rc,_,prompted=interactive(['import','--file',str(nodefile)],b'yes\n')
            check(rc==0 and prompted and status()['node_count']==3)
            run('remove','SG1');check(status()['node_count']==2)
            run('restore');check(status()['node_count']==3)
            run('copy-resource');check(token in clip.read_text() and 'tag=dohsub' in clip.read_text())
            write(clip,line.replace('US1','JP1'))
            run('add','-y');check(status()['node_count']==4 and clip.read_text()=='')
            # Failed persistence must not change the active snapshot.
            runtime=tmp/'state/runtime.json';saved_runtime=tmp/'runtime.saved'
            runtime.rename(saved_runtime);runtime.symlink_to(tmp/'blocked-write')
            write(nodefile,line.replace('US1','FAIL1'))
            run('import','--file',nodefile,'-y',code=1)
            check(status()['node_count']==4)
            runtime.unlink();saved_runtime.rename(runtime)
            run('remove','FAIL1')
            # Private file modes and exclusive startup.
            check((tmp/'state').stat().st_mode&0o777==0o700)
            check(all(p.stat().st_mode&0o077==0 for p in (tmp/'state').iterdir()))
            run('start',code=1)
            # A cached negative deadline survives a restart; no immediate negative re-query.
            negative_before=(tmp/'dns.log').read_text().splitlines().count('missing.example.com')
            run('restart')
            check(status()['dns_retries']['missing.example.com']['retry_in_seconds']>0)
            check((tmp/'dns.log').read_text().splitlines().count('missing.example.com')==negative_before)
            # A newly failing host uses the short transient schedule, not the negative one.
            write(tmp/'fail-dns','fail')
            write(nodefile,line.replace('one.example.com','flaky.example.com').replace('US1','KR1'))
            run('import','--file',nodefile,'-y')
            wait_for(lambda:'flaky.example.com' in status()['dns_retries'])
            retry=status()['dns_retries']['flaky.example.com']
            check(retry['reason']=='transient' and 24<=retry['retry_in_seconds']<=30)
            run('remove','KR1')
            pid=status()['pid']
            ps=subprocess.run(['ps','-o','rss=','-p',str(pid)],capture_output=True,text=True)
            if ps.returncode==0:print('Isolated '+BINARY.parent.name+' daemon RSS:',ps.stdout.strip(),'KiB (single synthetic-node sample)')
            run('config','set','--doh','https://changed.example/query')
            run('restart',code=1,extra={'TEST_DOH_FAIL':'1'});check(status()['pid']==pid)
            run('restart');check(status()['pid']!=pid)
            for tag in ['US1','TW1','SG1','JP1']:run('remove',tag)
            check(http_get(status()['port'],path)[0]==503)
            check(b'Health: awaiting_import' in run('status').stdout)
            run('restore');check(status()['node_count']==1)
            run('stop');run('stop');check(not (tmp/'state/control.sock').exists())
            stopped=run('status').stdout
            check(b'Service: stopped' in stopped and b'Listening: none' in stopped and b'Staged nodes: 1' in stopped and b'Saved DoH: configured' in stopped)
            config_path=tmp/'state/config.json';saved_config=tmp/'config.saved'
            config_path.rename(saved_config);config_path.symlink_to(tmp/'missing-config-target')
            r=run('start','--doh','https://dns.example/query',code=1)
            check(b'newly started daemon was stopped' in r.stderr and not (tmp/'state/control.sock').exists())
            config_path.unlink();saved_config.rename(config_path)
            # Standalone installer stages support files and leaves old binaries on failure.
            prefix=tmp/'prefix with spaces'
            out=subprocess.run(['bash',str(ROOT/'install.sh'),'--binary',str(BINARY),'--prefix',str(prefix)],env=env,capture_output=True,timeout=20)
            assert out.returncode==0,out.stderr.decode()
            installed=prefix/'bin/dohsub';check(run('--version',binary=installed).stdout==v)
            for part in ['share/man/man1/dohsub.1','share/bash-completion/completions/dohsub','share/zsh/site-functions/_dohsub','share/fish/vendor_completions.d/dohsub.fish']:check((prefix/part).is_file())
            oldhash=hashlib.sha256(installed.read_bytes()).hexdigest()
            bad_resource=tmp/'bad-resource'
            write(bad_resource,'#!/bin/sh\nif [ "$1" = man ]; then exit 1; fi\nexec '+shlex.quote(str(BINARY))+' "$@"\n',0o755)
            out=subprocess.run(['bash',str(ROOT/'install.sh'),'--binary',str(bad_resource),'--prefix',str(prefix)],env=env,capture_output=True,timeout=20)
            check(out.returncode!=0 and hashlib.sha256(installed.read_bytes()).hexdigest()==oldhash and b'Manual generation failed' in out.stderr)
            bad=tmp/'bad';write(bad,'#!/bin/sh\nexit 1\n',0o755)
            out=subprocess.run(['bash',str(ROOT/'install.sh'),'--binary',str(bad),'--prefix',str(prefix)],env=env,capture_output=True,timeout=20)
            check(out.returncode!=0 and hashlib.sha256(installed.read_bytes()).hexdigest()==oldhash)
            # Mock stable release and every installer download; no real update/network.
            asset=tmp/'asset';sums=tmp/'SHA256SUMS'
            wrapper='#!/bin/sh\nif [ "$1" = --version ] || [ "$1" = version ]; then printf "dohsub version 1.0.0 (2026-11-11)\\n'+REPO+'\\n"; else exec '+shlex.quote(str(BINARY))+' "$@"; fi\n'
            write(asset,wrapper,0o755)
            platform=('aarch64' if os.uname().machine in ['arm64','aarch64'] else 'x86_64')+('-apple-darwin' if sys.platform=='darwin' else '-unknown-linux-gnu')
            name='dohsub-'+platform
            write(sums,hashlib.sha256(asset.read_bytes()).hexdigest()+'  '+name+'\n'+hashlib.sha256((ROOT/'install.sh').read_bytes()).hexdigest()+'  install.sh\n')
            env.update(TEST_ASSET=str(asset),TEST_SUMS=str(sums),TEST_INSTALLER=str(ROOT/'install.sh'),TEST_PLATFORM=platform)
            check(b'failed' in run('upgrade',code=1,binary=installed,extra={'TEST_RELEASE_FAIL':'1'}).stderr)
            run('upgrade',code=1,binary=installed,extra={'TEST_PRERELEASE':'1'})
            check(b'newer than' in run('upgrade',binary=installed,extra={'TEST_TAG':'v0.0.9'}).stdout)
            run('upgrade',code=1,binary=installed) # noninteractive requires --yes
            for reply in [b'n\n',b'\n',b'garbage\n',b'\x04']:
                rc,_,prompted=interactive(['upgrade'],reply,binary=installed)
                check(rc==0 and prompted and hashlib.sha256(installed.read_bytes()).hexdigest()==oldhash)
            original=sums.read_text();write(sums,'0'*64+'  '+name+'\n')
            run('upgrade','-y',code=1,binary=installed)
            check(hashlib.sha256(installed.read_bytes()).hexdigest()==oldhash)
            out=subprocess.run(['bash',str(ROOT/'install.sh'),'--version','1.0.0','--prefix',str(prefix)],env=env,capture_output=True,timeout=20)
            check(out.returncode!=0 and hashlib.sha256(installed.read_bytes()).hexdigest()==oldhash)
            write(sums,original)
            remote_prefix=tmp/'remote-prefix'
            out=subprocess.run(['bash',str(ROOT/'install.sh'),'--version','1.0.0','--prefix',str(remote_prefix)],env=env,capture_output=True,timeout=20)
            check(out.returncode==0 and b'version 1.0.0' in run('--version',binary=remote_prefix/'bin/dohsub').stdout)
            piped_prefix=tmp/'piped-prefix'
            out=subprocess.run(['bash','-s','--','--prefix',str(piped_prefix)],input=(ROOT/'install.sh').read_bytes(),env=env,capture_output=True,timeout=20)
            check(out.returncode==0 and b'version 1.0.0' in run('--version',binary=piped_prefix/'bin/dohsub').stdout)
            # Explicit remote installer pin must reject a binary with a different version.
            out=subprocess.run(['bash',str(ROOT/'install.sh'),'--version','2.0.0','--prefix',str(prefix)],env=env,capture_output=True,timeout=20)
            check(out.returncode!=0 and hashlib.sha256(installed.read_bytes()).hexdigest()==oldhash)
            # Native Homebrew ownership/lag checks are mocked, never use the user's brew.
            cellar=tmp/'Cellar/dohsub'/VERSION;(cellar/'bin').mkdir(parents=True)
            brew_binary=cellar/'bin/dohsub';shutil.copy2(BINARY,brew_binary)
            write(fakebin/'brew', '\n'.join([
                '#!/usr/bin/env python3',
                'import json,os,shutil,sys',
                'a=sys.argv[1:]',
                'if a[0]=="--prefix":print(os.environ["TEST_BREW_ROOT"])',
                'elif a[0]=="info":print(json.dumps({"formulae":[{"full_name":"leo1394/dohsub/dohsub","versions":{"stable":os.environ.get("TEST_BREW_STABLE","0.9.0")}}]}))',
                'elif a[0]=="update":sys.exit(0)',
                'elif a[0]=="upgrade":',
                ' if os.environ.get("TEST_BREW_FAIL"):sys.exit(1)',
                ' shutil.copy2(os.environ["TEST_ASSET"],os.environ["TEST_BREW_ROOT"]+"/bin/dohsub")',
                'else:sys.exit(1)',
                ''
            ]),0o755)
            brew_env={'TEST_BREW_ROOT':str(cellar)}
            r=run('upgrade','-y',binary=brew_binary,code=1,extra=brew_env)
            check(b'not yet available' in r.stderr)
            r=run('upgrade','-y',binary=brew_binary,code=1,extra=brew_env|{'TEST_BREW_STABLE':'1.0.0','TEST_BREW_FAIL':'1'})
            check(b'Homebrew upgrade failed' in r.stderr)
            r=run('upgrade','-y',binary=brew_binary,extra=brew_env|{'TEST_BREW_STABLE':'1.0.0'})
            check(b'Successfully upgraded' in r.stdout)
            run('upgrade','-y',binary=installed)
            check(b'version 1.0.0' in run('--version',binary=installed).stdout and (prefix/'bin/dohsub.previous').exists())
            # Post-install verification failure restores the previous executable.
            rollback=tmp/'rollback'
            write(rollback,'#!/bin/sh\ncase "$0" in */bin/dohsub) exit 1;; esac\nexec '+shlex.quote(str(BINARY))+' "$@"\n',0o755)
            before=installed.read_bytes()
            out=subprocess.run(['bash',str(ROOT/'install.sh'),'--binary',str(rollback),'--prefix',str(prefix)],env=env,capture_output=True,timeout=20)
            check(out.returncode!=0 and installed.read_bytes()==before and b'restored' in out.stderr)
            if shutil.which('man'):
                e=env|{'MANPATH':str(prefix/'share/man')+':','MANPAGER':'cat','PAGER':'cat'}
                found=subprocess.run(['man','-w','dohsub'],env=e,capture_output=True)
                check(found.returncode==0 and b'dohsub.1' in found.stdout)
            if shutil.which('mandoc'):
                r=subprocess.run(['mandoc','-T','ascii',str(prefix/'share/man/man1/dohsub.1')],capture_output=True)
                rendered=re.sub(rb'.\x08', b'', r.stdout)
                check(r.returncode==0 and b'EXAMPLES' in rendered and b'CAVEATS' in rendered)
        finally:
            subprocess.run([str(BINARY),'stop'],env=env,capture_output=True,timeout=20)
    print(f'Passed {checks} behavioral assertions (isolated mocks, no real nodes or services changed).')

if __name__=='__main__':main()
