use std::{io::{BufRead,BufReader,Write},process::{Child,Command,Stdio},sync::{mpsc,Arc,Mutex},thread,time::Duration};
use crate::{nodes::Node,Result,fail};
const ENGINE:&[u8]=include_bytes!(concat!(env!("OUT_DIR"),"/dohsub-relay"));
fn executable()->Result<std::path::PathBuf> {
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
pub struct RelayHealth {pub dns_state:String,pub dns_expires:u64,pub retry_at:u64,pub upstream:String,pub last_success:u64}
impl Default for RelayHealth {fn default()->Self{Self{dns_state:"pending".into(),dns_expires:0,retry_at:0,upstream:"unverified".into(),last_success:0}}}
impl RelayHealth {
    fn update(&mut self,value:&serde_json::Value) {
        match value["event"].as_str() {
            Some("dns")=>{
                if let (Some(state),Some(expires),Some(retry))=(value["dns_state"].as_str(),value["dns_expires"].as_u64(),value["retry_at"].as_u64()) {
                    if matches!(state,"pending"|"ready"|"refreshing"|"retrying") && expires<=crate::store::now()+3600 && retry<=crate::store::now()+60 {self.dns_state=state.into();self.dns_expires=expires;self.retry_at=retry;if expires>crate::store::now() && self.upstream=="dns_failed" {self.upstream="unverified".into();}}
                }
            },
            Some("upstream")=>if let Some(state)=value["upstream"].as_str() {
                if matches!(state,"dns_failed"|"interface_failed"|"tcp_failed"|"tcp_connected"|"handshake_failed"|"io_failed"|"verified") {self.upstream=state.into();if state=="verified" {self.last_success=crate::store::now();}}
            },
            _=>()
        }
    }
}
pub type Health=Arc<Mutex<std::collections::BTreeMap<u16,RelayHealth>>>;
pub struct Engines(Vec<Child>,pub Health);
impl Drop for Engines {fn drop(&mut self) {for child in &mut self.0 {let _=child.kill();let _=child.wait();}}}
impl Engines {
    pub fn start(nodes:&[Node],config:&crate::store::Config)->Result<Self> {
        if nodes.iter().any(|n|n.relay.is_some()) && config.interface.is_none() {return Err(fail("Relay needs --interface PHYSICAL_INTERFACE; no system-route fallback"));}
        if nodes.iter().any(|n|n.relay.is_some()) {
            let endpoint=url::Url::parse(config.doh.as_deref().ok_or_else(||fail("DoH is not configured"))?)?;
            if endpoint.host_str().and_then(|h|h.parse::<std::net::IpAddr>().ok()).is_none() {return Err(fail("Relay requires an IP-literal HTTPS DoH endpoint; no system DNS bootstrap"));}
        }
        let mut engines=Self(Vec::new(),Health::default());let mut ports=std::collections::HashSet::new();
        for node in nodes.iter().filter(|n|n.relay.is_some()) {
            if !ports.insert(node.port) {return Err(fail("Relay listen port collision; no engines left running"));}
            let exe=executable()?;
            let mut child=Command::new(exe).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|fail("Missing dohsub-relay helper; build/install it next to dohsub"))?;
            let input=serde_json::json!({"resolver":std::env::current_exe()?,"interface":config.interface,"listen_port":node.port,"username":node.options.iter().find(|(k,_)|k=="username").map(|(_,v)|v),"password":node.options.iter().find(|(k,_)|k=="password").map(|(_,v)|v),"node":node.relay});
            let mut stdin=child.stdin.take().unwrap();let stdout=child.stdout.take().unwrap();
            engines.0.push(child);
            stdin.write_all(serde_json::to_string(&input)?.as_bytes())?;drop(stdin);
            let health=engines.1.clone();let port=node.port;health.lock().unwrap().insert(port,RelayHealth::default());
            let (tx,rx)=mpsc::channel();thread::spawn(move||{
                let mut reader=BufReader::new(stdout);let mut line=String::new();let _=reader.read_line(&mut line);let _=tx.send(line.clone());
                loop {
                    line.clear();if reader.read_line(&mut line).unwrap_or(0)==0 {break;}
                    if line.len()<=4096 {if let Ok(value)=serde_json::from_str(&line) {if let Some(state)=health.lock().unwrap().get_mut(&port) {state.update(&value);}}}
                }
                if let Some(state)=health.lock().unwrap().get_mut(&port) {state.dns_expires=0;state.upstream="engine_exited".into();}
            });
            if rx.recv_timeout(Duration::from_secs(5)).ok().as_deref()!=Some("READY\n") {return Err(fail("Relay startup failed; no engines left running"));}
        }
        Ok(engines)
    }
}

#[cfg(test)] mod tests {
    use super::*;
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
