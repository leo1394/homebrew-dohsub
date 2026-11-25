#!/usr/bin/env python3
"""Isolated macOS launchd lifecycle regression; never touches the user's service."""
import json
import os
from pathlib import Path
import plistlib
import signal
import socket
import subprocess
import sys
import tempfile
import time
from behavior import BINARY, MOCK, write


def main():
    if sys.platform != 'darwin':
        print('launchd regression: skipped (macOS only)')
        return
    target = None
    with tempfile.TemporaryDirectory(prefix='ds-supervision-', dir='/private/tmp') as tmp:
        home = Path(tmp)
        home.chmod(0o700)
        fakebin = home / 'bin'; fakebin.mkdir()
        write(fakebin / 'curl', MOCK, 0o755)
        listener = socket.socket(); listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]; listener.close()
        write(home / 'config.json', json.dumps({'doh': 'https://dns.example/dns-query', 'port': port}))
        write(home / 'nodes.json', '[]')
        write(home / 'token', 'a' * 43)
        label = 'local.dohsub.test.' + str(os.getpid())
        target = f'gui/{os.getuid()}/{label}'
        write(home / 'launchd.json', json.dumps({'label': label}))
        env = dict(os.environ, DOHSUB_HOME=str(home), PATH=str(fakebin) + ':' + os.environ['PATH'], TEST_DNS_LOG=str(home/'dns.log'), TEST_PUBLIC_IPV4='93.184.216.34')
        plist = home / 'job.plist'
        plist.write_bytes(plistlib.dumps({'Label': label, 'ProgramArguments': [str(BINARY), 'run'], 'EnvironmentVariables': env, 'RunAtLoad': True, 'KeepAlive': {'SuccessfulExit': False}, 'ThrottleInterval': 1, 'StandardErrorPath': str(home/'service.log'), 'StandardOutPath': str(home/'service.log')}))
        def ctl(*args):
            return subprocess.run(['/bin/launchctl', *args], capture_output=True, timeout=15)
        def status():
            with socket.socket(socket.AF_UNIX) as s:
                s.settimeout(1); s.connect(str(home/'control.sock')); s.sendall(b'"status"\n')
                data = b''
                while b'\n' not in data:
                    chunk = s.recv(65536)
                    if not chunk: break
                    data += chunk
                return json.loads(data)
        def wait(predicate, timeout=15):
            end = time.monotonic() + timeout
            while time.monotonic() < end:
                try:
                    value = predicate()
                    if value: return value
                except (OSError, ValueError): pass
                time.sleep(.1)
            raise AssertionError('supervision lifecycle timed out')
        def command(*args):
            result = subprocess.run([str(BINARY), *args], env=env, capture_output=True, timeout=30)
            assert result.returncode == 0, result.stderr.decode()
        try:
            assert ctl('bootstrap', f'gui/{os.getuid()}', str(plist)).returncode == 0
            first = wait(lambda: status())
            assert first['port'] == port
            job = ctl('print', target).stdout.decode()
            assert f'pid = {first["pid"]}' in job
            os.kill(first['pid'], signal.SIGKILL)
            second = wait(lambda: (v if (v := status())['pid'] != first['pid'] else None))
            assert second['port'] == port
            command('restart')
            third = wait(lambda: (v if (v := status())['pid'] != second['pid'] else None))
            assert f'pid = {third["pid"]}' in ctl('print', target).stdout.decode()
            command('stop')
            time.sleep(3)
            try: status()
            except OSError: pass
            else: raise AssertionError('explicit stop was automatically undone')
            command('start')
            fourth = wait(lambda: status())
            assert f'pid = {fourth["pid"]}' in ctl('print', target).stdout.decode()
            assert fourth['port'] == port and (home/'token').read_text() == 'a' * 43
            assert json.loads((home/'nodes.json').read_text()) == []
            print('launchd regression: foreground PID, crash recovery, supervised restart/start, intentional stop, stable port/token/nodes passed')
        finally:
            ctl('bootout', target)


if __name__ == '__main__':
    main()
