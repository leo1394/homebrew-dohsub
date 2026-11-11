use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, BTreeSet}, fs::{self, File, OpenOptions}, io::{BufRead, BufReader, Read, Write}, net::{TcpListener}, os::{fd::{AsRawFd, FromRawFd, RawFd}, unix::{fs::{MetadataExt, OpenOptionsExt, PermissionsExt}, process::CommandExt, net::{UnixListener, UnixStream}}}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::{Arc, Mutex, Condvar, atomic::{AtomicBool, Ordering}}, thread, time::{Duration, Instant}};
use tiny_http::{Response, StatusCode, Header};
use crate::{Result, fail, dns, nodes::{Node, public_ip}, store::{self, Config, Lock}};

#[derive(Clone, Serialize, Deserialize)]
pub struct Runtime { pub config: Config, pub nodes: Vec<Node> }
#[derive(Clone, Serialize, Deserialize)]
pub struct Address { pub ip: String, pub expires: u64 }
#[derive(Clone, Serialize, Deserialize)]
pub struct Retry { pub failures: u32, pub next: u64, pub negative: bool }
#[derive(Clone, Default, Serialize, Deserialize)]
struct Saved { doh: Option<String>, cache: BTreeMap<String, Address>, retries: BTreeMap<String, Retry> }
struct State { runtime: Runtime, saved: Saved, revision: u64, force: bool, fetches: u64 }
type Shared = Arc<(Mutex<State>, Condvar)>;
#[derive(Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Request { Status, Reload, Refresh, Stop }
pub fn request(dir: &Path, op: Request) -> Result<Value> {
    let mut stream=UnixStream::connect(dir.join("control.sock")).map_err(|error| std::io::Error::new(error.kind(),"dohsub is not running"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?; stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    serde_json::to_writer(&mut stream,&op)?; stream.write_all(b"\n")?;
    let mut line=String::new(); BufReader::new(stream.take(1_048_577)).read_line(&mut line)?;
    let value:Value=serde_json::from_str(&line).map_err(|_| fail("Invalid daemon response"))?;
    if let Some(error)=value.get("error").and_then(Value::as_str) { return Err(fail(error)); }
    Ok(value)
}
pub fn running(dir: &Path) -> bool { request(dir,Request::Status).is_ok() }
pub fn choose_listener(port: u16, explicit: bool) -> Result<TcpListener> {
    for candidate in port..=u16::MAX {
        match TcpListener::bind(("127.0.0.1",candidate)) {
            Ok(listener)=>return Ok(listener),
            Err(e) if e.kind()==std::io::ErrorKind::AddrInUse && !explicit=>continue,
            Err(_)=>return Err(fail("Requested local port is occupied or cannot be bound; start aborted"))
        }
    }
    Err(fail("No free loopback port available"))
}
pub fn start(dir: &Path, mut config: Config, explicit: bool) -> Result<u16> {
    store::init(dir)?;
    let lock=Lock::acquire(&dir.join("daemon.lock"),false)?;
    let nodes:Vec<Node>=store::load(&dir.join("nodes.json"))?;
    let host=nodes.iter().find(|n| !public_ip(&n.host)).map(|n|n.host.as_str()).unwrap_or("example.com");
    dns::preflight(&config,host)?;
    let listener=choose_listener(config.port,explicit)?;
    config.port=listener.local_addr()?.port();
    let runtime=Runtime {config:config.clone(),nodes};
    let token=store::token(dir)?;
    let _=token;
    store::save(&dir.join("runtime.json"),&runtime)?;
    let logpath=dir.join("service.log");
    if logpath.is_symlink() { return Err(fail("Log cannot be a symlink")); }
    let log=OpenOptions::new().create(true).append(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(&logpath)?;
    let m=log.metadata()?;if !m.is_file() || m.uid()!=unsafe{libc::geteuid()} || m.mode() & 0o077!=0 {return Err(fail("Log must be a private regular file owned by you"));}
    let fd=listener.as_raw_fd(); let lockfd=lock.fd();
    let flags=unsafe {libc::fcntl(fd,libc::F_GETFD)}; let lockflags=unsafe {libc::fcntl(lockfd,libc::F_GETFD)};
    if flags < 0 || lockflags < 0 { return Err(fail("Cannot prepare daemon descriptors")); }
    unsafe {libc::fcntl(fd,libc::F_SETFD,flags & !libc::FD_CLOEXEC); libc::fcntl(lockfd,libc::F_SETFD,lockflags & !libc::FD_CLOEXEC);}
    let mut command=Command::new(std::env::current_exe()?);
    unsafe {command.pre_exec(||{if libc::setsid()<0 {return Err(std::io::Error::last_os_error());}Ok(())});}
    let spawn=command.args(["__serve","--fd",&fd.to_string(),"--lock-fd",&lockfd.to_string()]).env("DOHSUB_HOME",dir).stdin(Stdio::null()).stdout(Stdio::from(log.try_clone()?)).stderr(Stdio::from(log)).spawn();
    unsafe {libc::fcntl(fd,libc::F_SETFD,flags);libc::fcntl(lockfd,libc::F_SETFD,lockflags);}
    let mut child=spawn.map_err(|_| fail("Cannot spawn dohsub daemon"))?;
    let deadline=Instant::now()+Duration::from_secs(8);
    while Instant::now()<deadline {
        if let Ok(value)=request(dir,Request::Status) {
            if value["pid"].as_u64()==Some(child.id() as u64) {
                if store::save(&dir.join("config.json"),&config).is_err() {
                    let _=child.kill();let _=child.wait();let _=fs::remove_file(dir.join("control.sock"));
                    return Err(fail("Could not cache configuration; newly started daemon was stopped"));
                }
                return Ok(config.port);
            }
        }
        if child.try_wait()?.is_some() { return Err(fail("Daemon exited during startup; inspect private service.log")); }
        thread::sleep(Duration::from_millis(50));
    }
    let _=child.kill();let _=child.wait();
    Err(fail("Daemon startup timed out"))
}
fn available(state: &State) -> Vec<(&Node,String)> {
    let now=store::now();
    state.runtime.nodes.iter().filter_map(|n| {
        if public_ip(&n.host) {Some((n,n.host.clone()))} else {state.saved.cache.get(&n.host).filter(|r|public_ip(&r.ip) && now<=r.expires.saturating_add(900)).map(|r|(n,r.ip.clone()))}
    }).collect()
}
fn health(state: &State) -> Value {
    let nodes=available(state); let count=nodes.len();
    let status=if state.runtime.nodes.is_empty() {"awaiting_import"} else if count==0 {"unavailable"} else if count<state.runtime.nodes.len() {"partial"} else {"ready"};
    json!({"pid":std::process::id(),"port":state.runtime.config.port,"status":status,"node_count":state.runtime.nodes.len(),"published_node_count":count,"subscription_fetches":state.fetches,
        "nodes":state.runtime.nodes.iter().map(|n|json!({"tag":n.tag(),"host":n.host,"port":n.port,"published":nodes.iter().any(|(p,_)|p.tag()==n.tag())})).collect::<Vec<_>>(),
        "dns_retries":state.saved.retries.iter().map(|(h,r)|(h,json!({"failures":r.failures,"reason":if r.negative {"nxdomain_or_no_address"} else {"transient"},"retry_in_seconds":r.next.saturating_sub(store::now())}))).collect::<BTreeMap<_,_>>()})
}
fn retry_delay(negative: bool, failures: u32) -> u64 {
    let (base,cap) = if negative {(900u64,21600u64)} else {(30,900)};
    let ceiling=base.saturating_mul(1u64 << failures.saturating_sub(1).min(16)).min(cap);
    let mut byte=[0u8;1]; let _=File::open("/dev/urandom").and_then(|mut f|f.read_exact(&mut byte));
    ceiling*(800+u64::from(byte[0])*200/255)/1000
}
fn scheduler(dir: PathBuf, shared: Shared, stopped: Arc<AtomicBool>) {
    loop {
        let (config,hosts,revision)={
            let (mutex,cv)=&*shared; let mut s=mutex.lock().unwrap();
            loop {
                if stopped.load(Ordering::Relaxed) {return;}
                let hosts=s.runtime.nodes.iter().filter(|n|!public_ip(&n.host)).map(|n|n.host.clone()).collect::<BTreeSet<_>>();
                let now=store::now();
                let next=hosts.iter().map(|h| s.saved.cache.get(h).map(|v|v.expires).unwrap_or(0).max(s.saved.retries.get(h).map(|v|v.next).unwrap_or(0))).min().unwrap_or(u64::MAX);
                if s.force || next<=now {
                    let force=s.force; s.force=false;
                    let due=hosts.into_iter().filter(|h|force || (s.saved.cache.get(h).map(|v|v.expires).unwrap_or(0)<=now && s.saved.retries.get(h).map(|v|v.next).unwrap_or(0)<=now)).collect::<Vec<_>>();
                    break (s.runtime.config.clone(),due,s.revision);
                }
                let wait=if next==u64::MAX {Duration::from_secs(86400)} else {Duration::from_secs(next.saturating_sub(now).min(86400))};
                s=cv.wait_timeout(s,wait).unwrap().0;
            }
        };
        for host in hosts {
            if stopped.load(Ordering::Relaxed) {return;}
            let answer=dns::resolve(&config,&host);
            let mut s=shared.0.lock().unwrap();
            if s.revision!=revision {break;}
            let now=store::now();
            match answer {
                Ok((ips,ttl))=>{
                    let old=s.saved.cache.get(&host).map(|r|r.ip.clone());
                    let ip=old.filter(|ip|ips.contains(ip)).unwrap_or_else(||ips[0].clone());
                    s.saved.cache.insert(host.clone(),Address {ip,expires:now.saturating_add(ttl)});
                    s.saved.retries.remove(&host);
                },
                Err(e)=>{
                    let failures=s.saved.retries.get(&host).map(|r|r.failures+1).unwrap_or(1).min(16);
                    let negative=e.reason==dns::Reason::Negative;
                    s.saved.retries.insert(host.clone(),Retry {failures,next:now.saturating_add(retry_delay(negative,failures)),negative});
                }
            }
            if store::save(&dir.join("dns-cache.json"),&s.saved).is_err() { eprintln!("Could not persist DNS cache; continuing with memory state"); }
        }
    }
}
fn same_path(a: &str,b: &str)->bool {a.len()==b.len() && a.bytes().zip(b.bytes()).fold(0u8,|v,(x,y)|v|(x^y))==0}
fn http_server(server: Arc<tiny_http::Server>, shared: Shared, token: String, stopped: Arc<AtomicBool>) {
    while !stopped.load(Ordering::Relaxed) {
        let Ok(request)=server.recv() else {break;};
        let mut s=shared.0.lock().unwrap();
        let valid_host=format!("127.0.0.1:{}",s.runtime.config.port);
        let localhost=format!("localhost:{}",s.runtime.config.port);
        let host=request.headers().iter().find(|h|h.field.equiv("Host")).map(|h|h.value.as_str());
        let origin=request.headers().iter().any(|h|h.field.equiv("Origin"));
        let (code,body,content_type)=if (host!=Some(valid_host.as_str()) && host!=Some(localhost.as_str())) || origin {(403,"Forbidden\n".into(),"text/plain")}
            else if request.method()!=&tiny_http::Method::Get && request.method()!=&tiny_http::Method::Head {(405,"Method not allowed\n".into(),"text/plain")}
            else if request.url()=="/health" {(200,health(&s).to_string(),"application/json")}
            else if same_path(request.url(),&format!("/sub/{}/nodes.conf",token)) {
                let nodes=available(&s);
                if nodes.is_empty() {(503,"No resolved nodes yet; retain previous subscription\n".into(),"text/plain")}
                else {let body=nodes.iter().map(|(n,ip)|n.render(ip)).collect::<Vec<_>>().join("\n")+"\n"; if request.method()==&tiny_http::Method::Get {s.fetches+=1;} (200,body,"text/plain; charset=utf-8")}
            } else {(404,"Not found\n".into(),"text/plain")};
        drop(s);
        let response=Response::from_string(body).with_status_code(StatusCode(code))
            .with_header(Header::from_bytes("Content-Type",content_type).unwrap())
            .with_header(Header::from_bytes("Cache-Control","no-store").unwrap())
            .with_header(Header::from_bytes("X-Content-Type-Options","nosniff").unwrap());
        let _=request.respond(response);
    }
}
pub unsafe fn serve(dir: &Path, fd: RawFd, lockfd: RawFd) -> Result<()> {
    if fd<3 || lockfd<3 || fd==lockfd {return Err(fail("Invalid inherited daemon descriptors"));}
    let _lock=unsafe {Lock::inherited(lockfd)};
    unsafe {libc::fcntl(fd,libc::F_SETFD,libc::FD_CLOEXEC);libc::fcntl(lockfd,libc::F_SETFD,libc::FD_CLOEXEC);}
    let listener=unsafe {TcpListener::from_raw_fd(fd)};
    if !listener.local_addr()?.ip().is_loopback() {return Err(fail("Refusing non-loopback listener"));}
    let runtime:Runtime=serde_json::from_slice(&store::read(&dir.join("runtime.json"))?).map_err(|_|fail("Invalid runtime snapshot"))?;
    let mut saved:Saved=store::load(&dir.join("dns-cache.json"))?;
    if saved.doh!=runtime.config.doh {saved=Saved {doh:runtime.config.doh.clone(),..Default::default()};}
    let hosts=runtime.nodes.iter().map(|n|n.host.clone()).collect::<BTreeSet<_>>();
    saved.retries.retain(|h,r|hosts.contains(h) && r.next<=store::now().saturating_add(21600));
    saved.cache.retain(|h,r|hosts.contains(h) && public_ip(&r.ip));
    let shared:Shared=Arc::new((Mutex::new(State {runtime,saved,revision:0,force:false,fetches:0}),Condvar::new()));
    let stopped=Arc::new(AtomicBool::new(false));
    let server=Arc::new(tiny_http::Server::from_listener(listener,None).map_err(|_|fail("Cannot start HTTP server"))?);
    let socket_path=dir.join("control.sock");
    if socket_path.exists() {fs::remove_file(&socket_path)?;}
    let control=UnixListener::bind(&socket_path)?;
    fs::set_permissions(&socket_path,fs::Permissions::from_mode(0o600))?;
    let worker={let sh=shared.clone();let st=stopped.clone();let d=dir.to_path_buf();thread::spawn(move||scheduler(d,sh,st))};
    let http={let sh=shared.clone();let st=stopped.clone();let se=server.clone();let tok=store::token(dir)?;thread::spawn(move||http_server(se,sh,tok,st))};
    for stream in control.incoming() {
        let mut stream=stream?;stream.set_read_timeout(Some(Duration::from_secs(2)))?;stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let mut line=String::new(); let read=BufReader::new((&stream).take(4096)).read_line(&mut line);
        let result=(||->Result<Value>{
            read?;
            let op:Request=serde_json::from_str(&line).map_err(|_|fail("Invalid control request"))?;
            let mut s=shared.0.lock().unwrap();
            match op {
                Request::Status=>Ok(health(&s)),
                Request::Stop=>{stopped.store(true,Ordering::Relaxed);shared.1.notify_all();Ok(json!({"stopped":true}))},
                Request::Reload|Request::Refresh=>{
                    let nodes:Vec<Node>=store::load(&dir.join("nodes.json"))?;
                    let force=matches!(op,Request::Refresh);
                    let runtime=Runtime {config:s.runtime.config.clone(),nodes};
                    let mut saved=s.saved.clone();
                    let hosts=runtime.nodes.iter().map(|n|n.host.clone()).collect::<BTreeSet<_>>();
                    saved.cache.retain(|h,_|hosts.contains(h));saved.retries.retain(|h,_|hosts.contains(h));
                    store::save(&dir.join("runtime.json"),&runtime)?;
                    store::save(&dir.join("dns-cache.json"),&saved)?;
                    s.runtime=runtime;s.saved=saved;s.revision+=1;s.force|=force;
                    let value=json!({"loaded_nodes":s.runtime.nodes.len(),"refresh_queued":force});
                    shared.1.notify_all();Ok(value)
                }
            }
        })();
        let body=result.unwrap_or_else(|_|json!({"error":"Control operation failed; no private input was logged"}));
        let _=serde_json::to_writer(&mut stream,&body);let _=stream.write_all(b"\n");
        if stopped.load(Ordering::Relaxed) {break;}
    }
    stopped.store(true,Ordering::Relaxed);shared.1.notify_all();server.unblock();
    let _=http.join();let _=worker.join();let _=fs::remove_file(socket_path);
    Ok(())
}
pub fn stop(dir: &Path)->Result<()> {
    request(dir,Request::Stop)?;
    let deadline=Instant::now()+Duration::from_secs(12);
    while Instant::now()<deadline {
        if Lock::acquire(&dir.join("daemon.lock"),false).is_ok() {return Ok(());}
        thread::sleep(Duration::from_millis(50));
    }
    Err(fail("Stop requested; current HTTPS request has not finished yet"))
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn port_collision_and_jitter_caps() {let a=choose_listener(0,true).unwrap();let p=a.local_addr().unwrap().port();assert!(choose_listener(p,true).is_err());if p<65535 {assert!(choose_listener(p,false).unwrap().local_addr().unwrap().port()>p);}for n in 1..20 {let v=retry_delay(true,n);assert!((720..=21600).contains(&v));let v=retry_delay(false,n);assert!((24..=900).contains(&v));}}
}
