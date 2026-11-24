# Explicit opt-in: DOHSUB_DOH_CONFIG and DOHSUB_PAPAYA_FILE must name private input files.
import os,pathlib,json,tempfile,subprocess,socket,urllib.request,time,hashlib
repo=pathlib.Path(__file__).resolve().parents[1];binary=pathlib.Path(os.environ.get('DOHSUB_TEST_BINARY',str(repo/'target/release/dohsub')))
source=pathlib.Path(os.environ['DOHSUB_DOH_CONFIG'])
with tempfile.TemporaryDirectory(prefix='dohsub-relay-accept-') as t:
 root=pathlib.Path(t);env=dict(os.environ,DOHSUB_HOME=str(root/'state'))
 def run(*args):
  r=subprocess.run([str(binary),*args],env=env,capture_output=True,text=True)
  if r.returncode:raise RuntimeError('dohsub '+args[0]+' failed: '+r.stderr)
  return r
 conf=json.loads(source.read_text());conf['interface']=os.environ['DOHSUB_TEST_INTERFACE'];state=root/'state';state.mkdir(mode=0o700)
 (state/'config.json').write_text(json.dumps(conf));(state/'config.json').chmod(0o600)
 nodes=json.loads(pathlib.Path(os.environ['DOHSUB_PAPAYA_FILE']).read_text())
 node=root/'one.json';node.write_text(json.dumps(nodes[:1]));node.chmod(0o600)
 old=root/'old.conf';old.write_text('shadowsocks=8.8.8.8:443, method=aes-128-gcm, password=fixture-only, tag=Legacy-Acceptance\n');old.chmod(0o600)
 run('import','--file',str(old));before=json.loads((state/'nodes.json').read_text())[0]
 run('import','--file',str(node));saved=json.loads((state/'nodes.json').read_text());assert saved[0]==before and len(saved)==2
 with socket.socket() as s:s.bind(('127.0.0.1',0));saved[1]['port']=s.getsockname()[1]
 (state/'nodes.json').write_text(json.dumps(saved));(state/'nodes.json').chmod(0o600)
 with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
 started=False
 try:
  run('start','--port',str(port));started=True
  assert list((state/'engines').glob('dohsub-relay-*')),'Bundled engine was not extracted'
  token=(state/'token').read_text().strip()
  text=urllib.request.urlopen('http://127.0.0.1:'+str(port)+'/sub/'+token+'/nodes.conf',timeout=5).read().decode()
  assert 'shadowsocks=8.8.8.8:443' in text and 'socks5=127.0.0.1:' in text
  relay=saved[1];options=dict(relay['options'])
  args=['curl','--silent','--show-error','--max-time','20','--proxy','socks5h://127.0.0.1:'+str(relay['port']),'--proxy-user',options['username']+':'+options['password'],'--output','/dev/null','--write-out','%{http_code}','https://example.com/']
  r=subprocess.run(args,capture_output=True,text=True);assert r.returncode==0 and r.stdout=='200',('HTTPS failed',r.returncode)
  bad=args.copy();bad[bad.index('--proxy-user')+1]='wrong:wrong';r=subprocess.run(bad,capture_output=True);assert r.returncode!=0
  run('refresh');assert json.loads((state/'nodes.json').read_text())[0]==before
  print('PASS: legacy record preserved; mixed subscription; authenticated fixed relay HTTPS 200; wrong credentials rejected; refresh preserved nodes')
 finally:
  if started:run('stop')
 if started:
  try:
   with socket.create_connection(('127.0.0.1',relay['port']),timeout=.2):raise AssertionError('relay still listening')
  except OSError:pass
  print('PASS: daemon stop also stops relay listener')
