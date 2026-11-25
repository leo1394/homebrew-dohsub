"""Isolated helper regression: more than 32 simultaneous SOCKS sessions."""
import base64,json,socket,subprocess,sys,threading,time
with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
key=base64.b64encode(bytes(16)).decode();cfg={'resolver':'/usr/bin/false','interface':'en0','listen_port':port,'username':'test','password':'test','node':{'protocol':'papaya','host':'fixture.example','port':443,'auth':{'server_key':key,'user_key':key}}}
p=subprocess.Popen([sys.argv[1]],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True);held=[]
try:
 p.stdin.write(json.dumps(cfg));p.stdin.close();assert p.stdout.readline().strip()=='READY'
 def drain():
  for line in p.stdout:pass
 threading.Thread(target=drain,daemon=True).start()
 for i in range(40):
  s=socket.create_connection(('127.0.0.1',port),timeout=2);held.append(s);s.sendall(b'\x05\x01\x02')
  try:reply=s.recv(2)
  except OSError:raise AssertionError('Connection %d reset'%(i+1)) from None
  if reply!=b'\x05\x02':raise AssertionError('Connection %d refused'%(i+1))
 print('PASS: 40 simultaneous SOCKS sessions accepted')
finally:
 for s in held:s.close()
 p.terminate();p.wait(timeout=5)
