#!/usr/bin/env python3
"""Exercise completion candidates. Does not run application operations."""
import os
from pathlib import Path
import pty
import signal
import select
import shlex
import shutil
import subprocess
import tempfile
import time

ROOT=Path(__file__).resolve().parents[1]
BINARY=Path(os.environ.get('DOHSUB_TEST_BINARY',ROOT/'target/debug/dohsub')).resolve()

def main():
    checks=0
    with tempfile.TemporaryDirectory(prefix='dsc-',dir='/private/tmp' if os.uname().sysname=='Darwin' else '/tmp') as tmp:
        tmp=Path(tmp);env=dict(os.environ,DOHSUB_HOME=str(tmp/'untouched'))
        for shell,name in [('bash','dohsub.bash'),('zsh','_dohsub'),('fish','dohsub.fish')]:
            out=subprocess.check_output([str(BINARY),'completion',shell],env=env)
            (tmp/name).write_bytes(out)
            assert out
        fixture=tmp/'node file.json';fixture.write_text('fixture')
        folder=tmp/'node folder';folder.mkdir()
        def bash(words):
            nonlocal checks
            body='source '+shlex.quote(str(tmp/'dohsub.bash'))+'\nCOMP_WORDS=('+ ' '.join(shlex.quote(w) for w in words)+')\nCOMP_CWORD='+str(len(words)-1)+'\n_dohsub dohsub "${COMP_WORDS[COMP_CWORD]}" "${COMP_WORDS[COMP_CWORD-1]}"\nprintf "%s\\0" "${COMPREPLY[@]}"\n'
            out=subprocess.check_output(['bash','--noprofile','--norc','-c',body],env=env)
            checks+=1;return [p.decode() for p in out.split(b'\0') if p]
        assert 'restart' in bash(['dohsub','re'])
        assert 'set' in bash(['dohsub','config','s'])
        assert '--import' in bash(['dohsub','start','--im'])
        assert 'zsh' in bash(['dohsub','completion','zs'])
        assert 'config' in bash(['dohsub','help','conf'])
        assert 'set' in bash(['dohsub','help','config','s'])
        assert str(fixture) in bash(['dohsub','import','--file',str(tmp/'node f')])
        assert str(folder) in bash(['dohsub','import','--file',str(tmp/'node f')])
        assert str(fixture) in bash(['dohsub','start','--port','55355','--import',str(tmp/'node f')])
        assert str(fixture) in bash(['dohsub','import','--file','old','--file',str(tmp/'node f')])
        assert '--file='+str(fixture) in bash(['dohsub','import','--file='+str(tmp/'node f')])
        for flag in ['--doh','--domain','--port','--interface']:
            assert not bash(['dohsub','start',flag,str(tmp/'node')])
        assert not bash(['dohsub','remove','--','--h'])
        if shutil.which('zsh'):
            pid,master=pty.fork()
            if pid==0:
                os.execvpe('zsh',['zsh','-f'],env|{'ZDOTDIR':str(tmp)})
            def drain(until=None,timeout=3):
                data=b'';deadline=time.monotonic()+timeout
                while time.monotonic()<deadline:
                    if select.select([master],[],[],.05)[0]:
                        try:data+=os.read(master,65536)
                        except OSError:break
                        if until and until in data:return data
                return data
            try:
                capture=tmp/'buffer'
                system_paths=subprocess.check_output(['zsh','-fc','print -l $fpath'],env=env|{'ZDOTDIR':str(tmp)},text=True).splitlines()
                trusted=[p for p in system_paths if p.startswith(('/usr/share/zsh/','/usr/lib/zsh/'))]
                init='fpath=('+shlex.quote(str(tmp))+' '+' '.join(map(shlex.quote,trusted))+'); autoload -Uz compinit; compinit -D -u; setopt nobeep; PROMPT="READY> "; function ds_capture() { print -r -- "$BUFFER" > '+shlex.quote(str(capture))+'; BUFFER=""; zle accept-line; }; zle -N ds_capture; bindkey "^X" ds_capture; bindkey "^I" expand-or-complete\n'
                ready=tmp/'ready'
                init=init.rstrip()+'; print ready > '+shlex.quote(str(ready))+'\n'
                startup=drain(timeout=.5)
                os.write(master,init.encode())
                deadline=time.monotonic()+5
                diag=b''
                while not ready.exists() and time.monotonic()<deadline:diag+=drain(timeout=.1)
                assert ready.exists(), ('Zsh initialization failed', startup[-500:],diag[-3000:])
                drain(timeout=.3)
                def zsh(line,expected):
                    nonlocal checks
                    if capture.exists():capture.unlink()
                    os.write(master,line.encode()+b'\t');time.sleep(.2);os.write(master,b'\x18')
                    deadline=time.monotonic()+4
                    while not capture.exists() and time.monotonic()<deadline:drain(timeout=.1)
                    assert capture.exists(), 'Zsh capture widget did not run'
                    result=capture.read_text().strip()
                    assert expected in result,(line,result,expected)
                    checks+=1;drain(b'READY> ',.1)
                zsh('dohsub config se','config set')
                zsh('dohsub completion zs','completion zsh')
                zsh('dohsub start --im','--import')
                zsh('dohsub help config se','config set')
                escaped=str(tmp/'node fi').replace(' ','\\ ')
                zsh('dohsub import --file '+escaped,'node\\ file.json')
                # URL arguments must not acquire a local file candidate.
                zsh('dohsub start --doh '+str(tmp/'node'),'--doh '+str(tmp/'node'))
                zsh('dohsub remove -- --h','remove -- --h')
            finally:
                os.write(master,b'\x03exit\n')
                deadline=time.monotonic()+2
                while time.monotonic()<deadline:
                    if os.waitpid(pid,os.WNOHANG)[0]:break
                    time.sleep(.05)
                else:os.kill(pid,signal.SIGKILL);os.waitpid(pid,0)
                os.close(master)
        else:print('UNVERIFIED: Zsh unavailable')
        if shutil.which('fish'):
            script='source '+shlex.quote(str(tmp/'dohsub.fish'))+'; complete -C "dohsub config s"; complete -C "dohsub completion zs"'
            out=subprocess.check_output(['fish','-c',script],env=env)
            assert b'set' in out and b'zsh' in out;checks+=2
        else:print('UNVERIFIED: Fish candidate execution (shell unavailable); Fish generation checked')
        assert not (tmp/'untouched').exists()
    print(f'Passed {checks} completion assertions; no state initialized.')

if __name__=='__main__':main()
