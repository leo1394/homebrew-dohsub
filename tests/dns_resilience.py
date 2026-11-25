#!/usr/bin/env python3
"""Exercise bounded DNS transport retries through the real resolver CLI."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get('DOHSUB_TEST_BINARY', ROOT/'target/release/dohsub')).resolve()
MOCK = '''#!/usr/bin/env python3
import base64,json,os,pathlib,socket,struct,sys,time,urllib.parse
p=pathlib.Path(os.environ['ATTEMPTS']);n=int(p.read_text())+1 if p.exists() else 1;p.write_text(str(n))
mode=os.environ['FAIL_MODE']
if mode=='transient' and n<3:sys.exit(28)
if mode=='permanent':sys.exit(28)
if mode=='slow':time.sleep(float(sys.argv[sys.argv.index('--max-time')+1]));sys.exit(28)
if mode=='http':sys.exit(22)
if mode=='malformed':sys.stdout.buffer.write(b'invalid dns');sys.exit()
url=sys.stdin.read().split('"')[1];wire=base64.urlsafe_b64decode(urllib.parse.parse_qs(urllib.parse.urlsplit(url).query)['dns'][0]+'===')
answer=struct.pack('!6H',0,0x8180,1,1,0,0)+wire[12:]+b'\\xc0\\x0c'+struct.pack('!HHIH',1,1,60,4)+socket.inet_aton('8.8.8.8')
sys.stdout.buffer.write(answer)
'''


def main():
    with tempfile.TemporaryDirectory(prefix='ds-dns-', dir='/private/tmp' if os.uname().sysname == 'Darwin' else '/tmp') as temp:
        root = Path(temp); root.chmod(0o700)
        (root/'curl').write_text(MOCK); (root/'curl').chmod(0o700)
        config = root/'config.json'
        config.write_text(json.dumps({'doh':'https://8.8.8.8/dns-query?token=fixture-private', 'interface':'en0'})); config.chmod(0o600)
        attempts = root/'attempts'
        for mode, expected_calls, success in [('transient',3,True),('permanent',3,False),('slow',2,False),('http',1,False),('malformed',1,False)]:
            attempts.unlink(missing_ok=True)
            env = dict(os.environ, DOHSUB_HOME=str(root), PATH=str(root)+':'+os.environ['PATH'], ATTEMPTS=str(attempts), FAIL_MODE=mode)
            started = time.monotonic()
            result = subprocess.run([str(BINARY),'__relay-resolve','--host','fixture.example','--interface','en0'],env=env,capture_output=True,text=True,timeout=13)
            assert (result.returncode == 0) == success, (mode, 'resolver outcome mismatch')
            assert int(attempts.read_text()) == expected_calls, (mode, 'unbounded or missing retries')
            assert time.monotonic()-started < 12
            assert 'fixture-private' not in result.stdout+result.stderr
            if success: assert json.loads(result.stdout)['ips'] == ['8.8.8.8']
        print('DNS resilience: transient recovery, bounded failures, no HTTP/malformed retries, private errors passed')


if __name__ == '__main__':
    main()
