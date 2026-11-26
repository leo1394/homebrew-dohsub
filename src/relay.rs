use std::{io::{BufRead,BufReader,Write},process::{Child,Command,Stdio},sync::{mpsc,Arc,Mutex},thread,time::Duration};
use crate::{nodes::Node,Result,fail};
const ENGINE:&[u8]=include_bytes!(concat!(env!("OUT_DIR"),"/dohsub-relay"));
pub(crate) fn executable()->Result<std::path::PathBuf> {
    use std::{fs::{self,OpenOptions},io::Read,os::unix::fs::{OpenOptionsExt,PermissionsExt,MetadataExt}};
    use sha2::{Digest,Sha256};
    let root=crate::store::home()?.join("engines");crate::store::init(&root)?;
    let digest=Sha256::digest(ENGINE).iter().map(|b|format!("{:02x}",b)).collect::<String>();
    let path=root.join(format!("dohsub-relay-{}",digest));
    let valid=(||->std::io::Result<bool>{
        let file=OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW|libc::O_NONBLOCK).open(&path)?;
        let m=file.metadata()?;if !m.is_file() || m.uid()!=unsafe{libc::geteuid()} || m.mode()&0o077!=0 || m.len()!=ENGINE.len() as u64 {return Ok(false);}
        let mut bytes=Vec::new();file.take(ENGINE.len() as u64+1).read_to_end(&mut bytes)?;Ok(bytes==ENGINE)
    })().unwrap_or(false);
    if !valid {crate::store::write(&path,ENGINE)?;}
    fs::set_permissions(&path,fs::Permissions::from_mode(0o700))?;
    Ok(path)
}
#[derive(Clone,serde::Serialize)]
pub struct RelayHealth {pub dns_state:String,pub dns_expires:u64,pub retry_at:u64,pub upstream:String,pub last_success:u64,pub active:u64,pub peak:u64,pub limit:u64,pub rejected:u64,pub failures:std::collections::BTreeMap<String,u64>,pub closed:std::collections::BTreeMap<String,u64>,pub last_failure:u64,pub last_error:String,pub error_kind:String,pub dns_revision:u64,pub recovery_state:String,pub tcp_retry_at:u64,pub consecutive_failures:u64,pub suppressed:u64}
impl Default for RelayHealth {fn default()->Self{Self{dns_state:"pending".into(),dns_expires:0,retry_at:0,upstream:"unverified".into(),last_success:0,active:0,peak:0,limit:0,rejected:0,failures:Default::default(),closed:Default::default(),last_failure:0,last_error:String::new(),error_kind:String::new(),dns_revision:0,recovery_state:"ready".into(),tcp_retry_at:0,consecutive_failures:0,suppressed:0}}}
impl RelayHealth {
    fn update(&mut self,value:&serde_json::Value) {
        match value["event"].as_str() {
            Some("recovery")=>{
                if let (Some(state),Some(retry),Some(failures),Some(suppressed))=(value["recovery_state"].as_str(),value["tcp_retry_at"].as_u64(),value["consecutive_failures"].as_u64(),value["suppressed"].as_u64()) {
                    if matches!(state,"ready"|"probing"|"cooldown"|"exhausted") && retry<=crate::store::now()+3601 && failures<=14 && suppressed>=self.suppressed {self.recovery_state=state.into();self.tcp_retry_at=retry;self.consecutive_failures=failures;self.suppressed=suppressed;}
                }
            },
            Some("capacity")=>{
                if let (Some(active),Some(peak),Some(limit),Some(rejected))=(value["active"].as_u64(),value["peak"].as_u64(),value["limit"].as_u64(),value["rejected"].as_u64()) {
                    if (16..=4096).contains(&limit) && active<=peak && peak<=limit && rejected>=self.rejected {self.active=active;self.peak=peak;self.limit=limit;self.rejected=rejected;}
                }
            },
            Some("closed")=>if let Some(reason)=value["reason"].as_str() {
                if matches!(reason,"eof"|"idle_timeout"|"upload_io_failed"|"download_io_failed") {
                    *self.closed.entry(reason.into()).or_default()+=1;
                    if reason.ends_with("_failed") {self.last_failure=crate::store::now();self.last_error=reason.into();*self.failures.entry(reason.into()).or_default()+=1;}
                }
            },
            Some("dns")=>{
                if let (Some(state),Some(expires),Some(retry))=(value["dns_state"].as_str(),value["dns_expires"].as_u64(),value["retry_at"].as_u64()) {
                    if matches!(state,"pending"|"ready"|"refreshing"|"retrying") && expires<=crate::store::now()+3600 && retry<=crate::store::now()+60 {self.dns_revision=value["dns_revision"].as_u64().unwrap_or(self.dns_revision);self.dns_state=state.into();self.dns_expires=expires;self.retry_at=retry;if expires>crate::store::now() && self.upstream=="dns_failed" {self.upstream="unverified".into();}}
                }
            },
            Some("upstream")=>if let Some(state)=value["upstream"].as_str() {
                if matches!(state,"dns_failed"|"interface_failed"|"tcp_failed"|"tcp_connected"|"handshake_failed"|"io_failed"|"verified") {self.upstream=state.into();if let Some(kind)=value["error_kind"].as_str() {if matches!(kind,"timeout"|"refused"|"unreachable"|"address_unavailable"|"resource_exhausted"|"other") {self.error_kind=kind.into();}}
                    if state.ends_with("_failed") {self.last_failure=crate::store::now();self.last_error=state.into();*self.failures.entry(state.into()).or_default()+=1;}else if state=="verified" {self.last_success=crate::store::now();}}
            },
            _=>()
        }
    }
}
pub type Health=Arc<Mutex<std::collections::BTreeMap<u16,RelayHealth>>>;
// Only validated public health fields are persisted; never raw helper output.
struct Diagnostics {dir:std::path::PathBuf,warned:bool}
impl Diagnostics {
    fn new(dir:std::path::PathBuf)->Self {Self{dir,warned:false}}
    fn append(&self,data:&[u8])->Result<()> {
        use std::{fs::{self,OpenOptions},os::unix::fs::{OpenOptionsExt,MetadataExt}};
        let path=self.dir.join("relay-diagnostics.jsonl");
        let mut file=OpenOptions::new().append(true).create(true).mode(0o600).custom_flags(libc::O_NOFOLLOW|libc::O_NONBLOCK).open(&path)?;
        let m=file.metadata()?;
        if !m.is_file() || m.uid()!=unsafe{libc::geteuid()} || m.mode()&0o077!=0 || m.nlink()!=1 {return Err(fail("Invalid private diagnostic file"));}
        if m.len()+data.len() as u64>1_048_576 {
            drop(file);fs::rename(&path,self.dir.join("relay-diagnostics.jsonl.1"))?;
            file=OpenOptions::new().append(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(path)?;
        }
        file.write_all(data)?;Ok(())
    }
    fn record(&mut self,port:u16,state:&RelayHealth) {
        let mut data=serde_json::to_vec(&serde_json::json!({"time":crate::store::now(),"pid":std::process::id(),"port":port,"health":state})).unwrap();data.push(b'\n');
        if self.append(&data).is_err() && !self.warned {self.warned=true;eprintln!("time={} relay diagnostics unavailable; check private log permissions and disk space",crate::store::now());}
    }
}
pub struct Engines(Vec<Child>,pub Health);
impl Drop for Engines {fn drop(&mut self) {for child in &mut self.0 {let _=child.kill();let _=child.wait();}}}
impl Engines {
    pub fn start(nodes:&[Node],config:&crate::store::Config)->Result<Self> {
        if !(16..=4096).contains(&config.relay_max_connections) {return Err(fail("relay_max_connections must be between 16 and 4096"));}
        if nodes.iter().any(|n|n.relay.is_some()) && config.interface.is_none() {return Err(fail("Relay needs --interface PHYSICAL_INTERFACE; no system-route fallback"));}
        if nodes.iter().any(|n|n.relay.is_some()) {
            let endpoint=url::Url::parse(config.doh.as_deref().ok_or_else(||fail("DoH is not configured"))?)?;
            if endpoint.host_str().and_then(|h|h.parse::<std::net::IpAddr>().ok()).is_none() {return Err(fail("Relay requires an IP-literal HTTPS DoH endpoint; no system DNS bootstrap"));}
        }
        let diagnostics=Arc::new(Mutex::new(Diagnostics::new(crate::store::home()?)));
        let mut engines=Self(Vec::new(),Health::default());let mut ports=std::collections::HashSet::new();
        for node in nodes.iter().filter(|n|n.relay.is_some()) {
            if !ports.insert(node.port) {return Err(fail("Relay listen port collision; no engines left running"));}
            let exe=executable()?;
            let mut child=Command::new(exe).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|fail("Missing dohsub-relay helper; build/install it next to dohsub"))?;
            let input=serde_json::json!({"resolver":std::env::current_exe()?,"interface":config.interface,"listen_port":node.port,"max_connections":config.relay_max_connections,"username":node.options.iter().find(|(k,_)|k=="username").map(|(_,v)|v),"password":node.options.iter().find(|(k,_)|k=="password").map(|(_,v)|v),"node":node.relay});
            let mut stdin=child.stdin.take().unwrap();let stdout=child.stdout.take().unwrap();
            engines.0.push(child);
            stdin.write_all(serde_json::to_string(&input)?.as_bytes())?;drop(stdin);
            let health=engines.1.clone();let port=node.port;health.lock().unwrap().insert(port,RelayHealth::default());
            let diagnostics=diagnostics.clone();
            let (tx,rx)=mpsc::channel();thread::spawn(move||{
                let mut last_log=0;let mut logged_failure=false;
                let mut reader=BufReader::new(stdout);let mut line=String::new();let _=reader.read_line(&mut line);let _=tx.send(line.clone());
                loop {
                    line.clear();if reader.read_line(&mut line).unwrap_or(0)==0 {break;}
                    if line.len()<=4096 {if let Ok(value)=serde_json::from_str(&line) {if let Some(state)=health.lock().unwrap().get_mut(&port) {let revision=state.dns_revision;let dns_retry=state.retry_at;state.update(&value);
                        let now=crate::store::now();let failed=state.last_failure>0;
                        if value["event"]=="recovery" || revision!=state.dns_revision || dns_retry!=state.retry_at || now.saturating_sub(last_log)>=30 || (failed && !logged_failure) {
                            diagnostics.lock().unwrap().record(port,state);last_log=now;logged_failure|=failed;
                        }
                    }}}
                }
                if let Some(state)=health.lock().unwrap().get_mut(&port) {state.dns_expires=0;state.upstream="engine_exited".into();diagnostics.lock().unwrap().record(port,state);}
            });
            if rx.recv_timeout(Duration::from_secs(5)).ok().as_deref()!=Some("READY\n") {return Err(fail("Relay startup failed; no engines left running"));}
        }
        Ok(engines)
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn recovery_health_rejects_untrusted_fields() {
        let mut state=RelayHealth::default();let retry=crate::store::now()+60;
        state.update(&serde_json::json!({"event":"recovery","recovery_state":"cooldown","tcp_retry_at":retry,"consecutive_failures":8,"suppressed":1000}));
        assert_eq!(state.recovery_state,"cooldown");assert_eq!(state.tcp_retry_at,retry);assert_eq!(state.suppressed,1000);
        state.update(&serde_json::json!({"event":"recovery","recovery_state":"secret","tcp_retry_at":0,"consecutive_failures":0,"suppressed":1001}));assert_eq!(state.recovery_state,"cooldown");
        state.update(&serde_json::json!({"event":"upstream","upstream":"tcp_failed","error_kind":"timeout"}));assert_eq!(state.error_kind,"timeout");
        state.update(&serde_json::json!({"event":"upstream","upstream":"tcp_failed","error_kind":"secret-url"}));assert_eq!(state.error_kind,"timeout");
        state.update(&serde_json::json!({"event":"recovery","recovery_state":"ready","tcp_retry_at":0,"consecutive_failures":0,"suppressed":1000}));assert_eq!(state.tcp_retry_at,0);assert_eq!(state.recovery_state,"ready");
        state.update(&serde_json::json!({"event":"recovery","recovery_state":"cooldown","tcp_retry_at":crate::store::now()+3600,"consecutive_failures":13,"suppressed":1000}));assert_eq!(state.consecutive_failures,13);
        state.update(&serde_json::json!({"event":"recovery","recovery_state":"exhausted","tcp_retry_at":0,"consecutive_failures":14,"suppressed":1001}));assert_eq!(state.recovery_state,"exhausted");assert_eq!(state.tcp_retry_at,0);
    }
    #[test] fn diagnostics_rotate_privately_and_refuse_links() {
        use std::{fs,os::unix::fs::{PermissionsExt,symlink}};
        let dir=std::env::temp_dir().join(format!("dohsub-diagnostics-{}",crate::store::random_token().unwrap()));crate::store::init(&dir).unwrap();
        let mut log=Diagnostics::new(dir.clone());let path=dir.join("relay-diagnostics.jsonl");
        log.record(12345,&RelayHealth::default());
        let row:serde_json::Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();assert_eq!(row["port"],12345);assert!(row["time"].as_u64().unwrap()>0);
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode()&0o777,0o600);
        crate::store::write(&path,&vec![b'x';1_048_576]).unwrap();log.append(b"{}\n").unwrap();assert_eq!(fs::read(&path).unwrap(),b"{}\n");assert_eq!(fs::metadata(dir.join("relay-diagnostics.jsonl.1")).unwrap().len(),1_048_576);
        fs::remove_file(&path).unwrap();let target=dir.join("secret");fs::write(&target,b"unchanged").unwrap();symlink(&target,&path).unwrap();assert!(log.append(b"bad").is_err());assert_eq!(fs::read(&target).unwrap(),b"unchanged");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn capacity_and_failure_history_survive_success() {
        let mut state=RelayHealth::default();
        state.update(&serde_json::json!({"event":"capacity","active":40,"peak":42,"limit":256,"rejected":3}));assert_eq!(state.active,40);assert_eq!(state.rejected,3);
        state.update(&serde_json::json!({"event":"capacity","active":300,"peak":42,"limit":256,"rejected":0}));assert_eq!(state.active,40);
        state.update(&serde_json::json!({"event":"upstream","upstream":"dns_failed"}));
        state.update(&serde_json::json!({"event":"closed","reason":"download_io_failed"}));
        state.update(&serde_json::json!({"event":"closed","reason":"idle_timeout"}));
        state.update(&serde_json::json!({"event":"upstream","upstream":"verified"}));
        assert_eq!(state.failures["dns_failed"],1);assert_eq!(state.failures["download_io_failed"],1);assert_eq!(state.failures.len(),2);assert_eq!(state.closed["idle_timeout"],1);assert!(state.last_failure>0);
    }
    #[test] fn health_only_accepts_bounded_public_status() {
        let mut state=RelayHealth::default();let now=crate::store::now();
        state.update(&serde_json::json!({"event":"dns","dns_state":"ready","dns_expires":now+60,"retry_at":0}));assert_eq!(state.dns_expires,now+60);
        state.update(&serde_json::json!({"event":"dns","dns_state":"ready","dns_expires":now+86400,"retry_at":0}));assert_eq!(state.dns_expires,now+60);
        state.update(&serde_json::json!({"event":"upstream","upstream":"private-value"}));assert_eq!(state.upstream,"unverified");
        state.update(&serde_json::json!({"event":"upstream","upstream":"tcp_connected"}));assert_eq!(state.last_success,0);
        state.update(&serde_json::json!({"event":"upstream","upstream":"verified"}));assert!(state.last_success>=now);
    }
    #[test] fn recovered_dns_does_not_keep_a_stale_dns_failure() {
        let mut state=RelayHealth::default();state.update(&serde_json::json!({"event":"upstream","upstream":"dns_failed"}));
        state.update(&serde_json::json!({"event":"dns","dns_state":"ready","dns_expires":crate::store::now()+60,"retry_at":0}));assert_eq!(state.upstream,"unverified");assert_eq!(state.last_success,0);
    }
}
