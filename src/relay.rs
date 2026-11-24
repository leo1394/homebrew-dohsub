use std::{io::{BufRead,BufReader,Write},process::{Child,Command,Stdio},sync::mpsc,thread,time::Duration};
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
pub struct Engines(Vec<Child>);
impl Drop for Engines {fn drop(&mut self) {for child in &mut self.0 {let _=child.kill();let _=child.wait();}}}
impl Engines {
    pub fn start(nodes:&[Node],config:&crate::store::Config)->Result<Self> {
        if nodes.iter().any(|n|n.relay.is_some()) && config.interface.is_none() {return Err(fail("Relay needs --interface PHYSICAL_INTERFACE; no system-route fallback"));}
        if nodes.iter().any(|n|n.relay.is_some()) {
            let endpoint=url::Url::parse(config.doh.as_deref().ok_or_else(||fail("DoH is not configured"))?)?;
            if endpoint.host_str().and_then(|h|h.parse::<std::net::IpAddr>().ok()).is_none() {return Err(fail("Relay requires an IP-literal HTTPS DoH endpoint; no system DNS bootstrap"));}
        }
        let mut engines=Self(Vec::new());let mut ports=std::collections::HashSet::new();
        for node in nodes.iter().filter(|n|n.relay.is_some()) {
            if !ports.insert(node.port) {return Err(fail("Relay listen port collision; no engines left running"));}
            let exe=executable()?;
            let mut child=Command::new(exe).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|fail("Missing dohsub-relay helper; build/install it next to dohsub"))?;
            let input=serde_json::json!({"resolver":std::env::current_exe()?,"interface":config.interface,"listen_port":node.port,"username":node.options.iter().find(|(k,_)|k=="username").map(|(_,v)|v),"password":node.options.iter().find(|(k,_)|k=="password").map(|(_,v)|v),"node":node.relay});
            let mut stdin=child.stdin.take().unwrap();let stdout=child.stdout.take().unwrap();
            engines.0.push(child);
            stdin.write_all(serde_json::to_string(&input)?.as_bytes())?;drop(stdin);
            let (tx,rx)=mpsc::channel();thread::spawn(move||{let mut line=String::new();let _=BufReader::new(stdout).read_line(&mut line);let _=tx.send(line);});
            if rx.recv_timeout(Duration::from_secs(5)).ok().as_deref()!=Some("READY\n") {return Err(fail("Relay startup failed; no engines left running"));}
        }
        Ok(engines)
    }
}
