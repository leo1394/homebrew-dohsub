#!/usr/bin/env python3
"""Real helper/daemon health propagation across short TTL and resolver outages."""
import base64
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
from behavior import BINARY, MOCK, write


def main():
    with tempfile.TemporaryDirectory(prefix='ds-health-', dir='/private/tmp' if os.uname().sysname == 'Darwin' else '/tmp') as tmp:
        root = Path(tmp); root.chmod(0o700)
        fakebin = root/'bin'; fakebin.mkdir()
        mock = MOCK.replace('if os.environ.get(\'TEST_DOH_FAIL\'):', "if Path(os.environ['FAIL_FLAG']).exists():").replace('3600,4)', '5,4)')
        write(fakebin/'curl', mock, 0o700)
        state = root/'state'; state.mkdir(mode=0o700)
        with socket.socket() as sock:
            sock.bind(('127.0.0.1',0)); port = sock.getsockname()[1]
        write(state/'config.json', json.dumps({'doh':'https://8.8.8.8/dns-query','port':port,'interface':'en0'}))
        key = base64.b64encode(bytes(16)).decode()
        write(root/'node.json',json.dumps([{'id':51,'protocol':'papaya','host':'fixture.example','port':443,'alias':'Health-Test','auth':{'server_key':key,'user_key':key}}]))
        flag = root/'fail'; flag.touch()
        env = dict(os.environ, DOHSUB_HOME=str(state), PATH=str(fakebin)+':'+os.environ['PATH'], FAIL_FLAG=str(flag), TEST_DNS_LOG=str(root/'dns.log'), TEST_PUBLIC_IPV4='8.8.8.8')
        def command(*args):
            result = subprocess.run([str(BINARY),*map(str,args)],env=env,capture_output=True,timeout=20)
            assert result.returncode == 0, 'CLI failed: '+args[0]
        def status():
            with socket.socket(socket.AF_UNIX) as sock:
                sock.settimeout(2);sock.connect(str(state/'control.sock'));sock.sendall(b'"status"\n');data=b''
                while b'\n' not in data:
                    chunk=sock.recv(65536)
                    if not chunk:break
                    data+=chunk
                return json.loads(data)
        def wait(predicate, seconds=15):
            deadline=time.monotonic()+seconds
            while time.monotonic()<deadline:
                value=status()
                if predicate(value):return value
                time.sleep(.1)
            raise AssertionError('Relay health did not reach expected state')
        command('import','--file',root/'node.json')
        # Foreground startup must survive unavailable DoH.
        process = subprocess.Popen([str(BINARY),'run'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        try:
            deadline=time.monotonic()+5
            while not (state/'control.sock').exists() and time.monotonic()<deadline:time.sleep(.05)
            wait(lambda v:v['relay_health'] and next(iter(v['relay_health'].values()))['dns_state']=='retrying')
            assert status()['published_node_count']==0, 'Listener alone was published as DNS-ready'
            flag.unlink()
            ready=wait(lambda v:v['published_node_count']==1)
            relay=next(iter(ready['relay_health'].values()))
            assert relay['upstream']=='unverified' and relay['last_success']==0
            initial=relay['dns_expires']
            wait(lambda v:next(iter(v['relay_health'].values()))['dns_expires']>initial,10)
            flag.touch()
            wait(lambda v:v['published_node_count']==0,15)
            assert process.poll() is None, 'DNS outage killed the service'
            flag.unlink()
            wait(lambda v:v['published_node_count']==1,15)
            command('stop');process.wait(timeout=15)
            assert process.returncode==0
            print('Relay health: cold outage visible, automatic recovery, proactive TTL refresh, expired DNS withheld, graceful stop passed')
        finally:
            if process.poll() is None:
                try:command('stop');process.wait(timeout=15)
                except Exception:process.kill();process.wait()


if __name__ == '__main__':
    main()
