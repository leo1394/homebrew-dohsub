use base64::{Engine, engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD}};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::{HashMap, HashSet}, net::Ipv4Addr};
use crate::{Result, fail};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node { pub host: String, pub port: u16, pub options: Vec<(String, String)>, #[serde(default)] pub relay: Option<Value> }
impl Node {
    pub fn tag(&self) -> &str { self.options.iter().find(|(k,_)| k == "tag").map(|(_,v)|v.as_str()).unwrap_or("") }
    pub fn render(&self, address: &str) -> String {
        let opts = self.options.iter().map(|(k,v)| format!("{}={}", k, if k == "tag" && self.relay.is_none() { format!("{} · AutoIP", v) } else { v.clone() })).collect::<Vec<_>>().join(", ");
        format!("{}={}:{}, {}", if self.relay.is_some() {"socks5"} else {"shadowsocks"}, address, self.port, opts)
    }
}
pub fn public_ip(host: &str) -> bool {
    let Ok(ip) = host.parse::<Ipv4Addr>() else { return false; };
    let [a,b,c,d] = ip.octets();
    !(ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_multicast() || ip.is_broadcast() || ip.is_documentation() || a == 0 || a >= 240 || (a == 100 && (64..=127).contains(&b)) || (a == 198 && (b == 18 || b == 19)) || (a == 192 && b == 0 && c == 0 && d != 9 && d != 10) || (a == 192 && b == 88 && c == 99))
}
fn host_valid(host: &str) -> bool {
    if host.parse::<Ipv4Addr>().is_ok() { return public_ip(host); }
    host.len() <= 253 && host.contains('.') && host.split('.').all(|s| !s.is_empty() && s.len() <= 63 && !s.starts_with('-') && !s.ends_with('-') && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'))
}
fn clean(s: &str) -> bool { !s.trim().is_empty() && !s.contains(',') && !s.chars().any(|c| c.is_control()) }
fn decode(s: &str) -> Result<String> {
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(s) { return String::from_utf8(bytes).map_err(|_| fail("Invalid SS link encoding")); }
    }
    Err(fail("Invalid SS link encoding"))
}
fn unquote(s: &str) -> Result<String> {
    let mut out = Vec::new(); let bytes = s.as_bytes(); let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() { return Err(fail("Invalid percent encoding")); }
            let digit = |b: u8| (b as char).to_digit(16).map(|n| n as u8);
            let hi=digit(bytes[i+1]).ok_or_else(||fail("Invalid percent encoding"))?;let lo=digit(bytes[i+2]).ok_or_else(||fail("Invalid percent encoding"))?;
            out.push(hi*16+lo); i += 3;
        } else { out.push(bytes[i]); i += 1; }
    }
    String::from_utf8(out).map_err(|_| fail("Invalid UTF-8 in SS link"))
}
fn uri(line: &str) -> Result<String> {
    let (body, tag) = line[5..].split_once('#').unwrap_or((&line[5..], "Shadowsocks"));
    if body.contains('?') { return Err(fail("Plugin SS links are unsupported; use full QX node text")); }
    let expanded = if body.contains('@') { body.to_string() } else { decode(body)? };
    let (auth, endpoint) = expanded.rsplit_once('@').ok_or_else(|| fail("SS link has no endpoint"))?;
    let auth = if auth.contains(':') { unquote(auth)? } else { decode(auth)? };
    let (method, password) = auth.split_once(':').ok_or_else(|| fail("SS link has no method/password"))?;
    let tag = unquote(tag)?;
    if !clean(password) || !clean(&tag) { return Err(fail("SS link fields cannot be represented safely")); }
    Ok(format!("shadowsocks={}, method={}, password={}, tag={}", endpoint.trim_end_matches('/'), method, password, tag))
}
pub fn parse_line(line: &str, domain: Option<&str>) -> Result<Node> {
    let expanded = if line.starts_with("ss://") { uri(line)? } else { line.to_string() };
    let (head, tail) = expanded.split_once(',').ok_or_else(|| fail("Expected a full QX Shadowsocks node or ss:// link"))?;
    let (kind, endpoint) = head.split_once('=').ok_or_else(|| fail("Invalid node endpoint"))?;
    if !kind.trim().eq_ignore_ascii_case("shadowsocks") { return Err(fail("Only Shadowsocks is supported")); }
    let (host, port) = endpoint.trim().rsplit_once(':').ok_or_else(|| fail("Missing node port"))?;
    let host = domain.unwrap_or(host).trim_end_matches('.').to_lowercase();
    let port: u16 = port.parse().map_err(|_| fail("Invalid node port"))?;
    if port == 0 || !host_valid(&host) { return Err(fail("Expected a DNS hostname or public IPv4 and a nonzero port")); }
    let mut options = Vec::new(); let mut keys = HashSet::new();
    for part in tail.split(',') {
        let (key,value) = part.split_once('=').ok_or_else(|| fail("Invalid QX option"))?;
        let key = key.trim().to_lowercase(); let mut value = value.trim().to_string();
        if key.is_empty() || !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') || !keys.insert(key.clone()) || !clean(&value) { return Err(fail("Duplicate or unsafe node option")); }
        if key == "tag" { value = value.trim_end_matches(" · AutoIP").trim().to_string(); if !clean(&value) { return Err(fail("Empty tag")); } }
        options.push((key,value));
    }
    for key in ["method", "password"] {
        let value = options.iter().find(|(k,_)| k == key).map(|(_,v)|v.as_str()).ok_or_else(|| fail("Missing method/password"))?;
        if value == "<redacted>" || (key == "method" && !value.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')) { return Err(fail("Invalid method or redacted password")); }
    }
    if !keys.contains("tag") { options.push(("tag".into(), format!("{}:{}", host, port))); }
    Ok(Node {host, port, options, relay: None})
}
fn json_node(value: &Value, domain: Option<&str>) -> Result<Option<Node>> {
    let obj = value.as_object().ok_or_else(|| fail("Expected a Shadowsocks node object"))?;
    for key in ["protocol","delivery"] {if obj.get(key).is_some_and(|v|!v.is_string()) {return Err(fail("Protocol/delivery must be strings"));}}
    match obj.get("enable") { Some(Value::Bool(false)) => return Ok(None), Some(v) if v.as_i64() == Some(0) => return Ok(None), None | Some(Value::Bool(true)) => (), Some(v) if v.as_i64() == Some(1) => (), _ => return Err(fail("Invalid enable flag")) }
    let protocol=obj.get("protocol").and_then(Value::as_str).unwrap_or("shadowsocks");
    if !["shadowsocks","ss2022","papaya"].contains(&protocol) {return Err(fail("Unsupported node protocol"));}
    let delivery=obj.get("delivery").and_then(Value::as_str).unwrap_or(if protocol=="papaya" {"relay"} else {"native"});
    if !["native","relay"].contains(&delivery) || (protocol=="papaya" && delivery!="relay") || (protocol=="shadowsocks" && delivery=="relay") {return Err(fail("Unsupported protocol/delivery combination"));}
    if obj.get("enable").is_some_and(|v|v==&Value::Bool(false) || v.as_i64()==Some(0)) {return Ok(None);}
    let method=obj.get("method").and_then(Value::as_str).unwrap_or("");
    if protocol=="shadowsocks" && obj.contains_key("host") {
        let mut normalized=obj.clone();normalized.remove("protocol");normalized.remove("delivery");normalized.remove("host");
        normalized.insert("hostname".into(),obj["host"].clone());normalized.insert("encryption".into(),Value::String(method.into()));
        return json_node(&Value::Object(normalized),domain);
    }
    if protocol=="ss2022" {
        let length=match method {"2022-blake3-aes-128-gcm"=>16,"2022-blake3-aes-256-gcm"=>32,_=>return Err(fail("SS2022 requires an explicit supported AES method"))};
        let password=obj.get("password").and_then(Value::as_str).ok_or_else(||fail("Missing SS2022 password"))?;
        for key in password.split(':') {if STANDARD.decode(key).map_or(true,|bytes|bytes.len()!=length) {return Err(fail("Invalid SS2022 key length or encoding"));}}
        if delivery=="native" {
            let mut normalized=obj.clone();normalized.remove("protocol");normalized.remove("delivery");
            normalized.insert("hostname".into(),obj.get("host").cloned().ok_or_else(||fail("Missing host"))?);
            normalized.insert("encryption".into(),Value::String(method.into()));
            normalized.remove("host");
            return json_node(&Value::Object(normalized),domain);
        }
    }
    if delivery=="relay" {
        if domain.is_some() {return Err(fail("--domain is unsupported for relay nodes"));}
        let host=obj.get("host").and_then(Value::as_str).filter(|h|host_valid(h)).ok_or_else(||fail("Invalid relay upstream host"))?;
        let port=obj.get("port").and_then(Value::as_u64).filter(|p|*p>0 && *p<=65535).ok_or_else(||fail("Invalid relay upstream port"))?;
        if obj.get("params").is_some_and(|p|!p.as_object().is_some_and(|o|o.is_empty())) {return Err(fail("Unsupported relay parameters"));}
        if protocol=="papaya" {
        for key in ["server_key","user_key"] {
            let text=obj.get("auth").and_then(|a|a.get(key)).and_then(Value::as_str).ok_or_else(||fail("Missing relay authentication"))?;
            let bytes=STANDARD.decode(text).map_err(|_|fail("Invalid relay authentication"))?;
            if bytes.len()!=16 && bytes.len()!=32 {return Err(fail("Invalid relay key length"));}
        }
        let server=STANDARD.decode(obj["auth"]["server_key"].as_str().unwrap()).unwrap();
        let user=STANDARD.decode(obj["auth"]["user_key"].as_str().unwrap()).unwrap();
        if server.len()!=user.len() {return Err(fail("Mismatched relay keys"));}
        }
        let id=obj.get("id").and_then(Value::as_u64).ok_or_else(||fail("Missing stable relay node id"))?;
        let alias=obj.get("alias").and_then(Value::as_str).filter(|v|clean(v)).unwrap_or("Relay");
        let tag=format!("{} · Relay-{}",alias,id);
        use sha2::{Digest,Sha256};
        let hash=Sha256::digest(format!("{}:{}:{}",host,port,id).as_bytes());
        let listen=40000+u16::from_be_bytes([hash[0],hash[1]])%20000;
        let mut listen=listen;let mut tag=tag;let mut username="dohsub".to_string();let mut password=crate::store::random_token()?;
        if let Some(listener)=obj.get("listener") {
            let listener=listener.as_object().ok_or_else(||fail("Invalid relay listener configuration"))?;
            if listener.keys().any(|k|!["port","username","password","tag"].contains(&k.as_str())) {return Err(fail("Unsupported relay listener field"));}
            if let Some(port)=listener.get("port") {listen=port.as_u64().filter(|p|*p>0 && *p<=65535).ok_or_else(||fail("Invalid listener port"))? as u16;}
            for (key,target) in [("username",&mut username),("password",&mut password),("tag",&mut tag)] {
                if let Some(value)=listener.get(key) {*target=value.as_str().filter(|v|clean(v) && *v!="<redacted>" && v.len()<=255).ok_or_else(||fail("Invalid listener option"))?.to_string();}
            }
        }
        let options=vec![("username".into(),username),("password".into(),password), ("udp-relay".into(),"false".into()),("tag".into(),tag)];
        let mut upstream=value.clone();upstream["protocol"]=Value::String(protocol.into());
        return Ok(Some(Node {host:"127.0.0.1".into(),port:listen,options,relay:Some(upstream)}));
    }
    match obj.get("enable") { Some(Value::Bool(false)) => return Ok(None), Some(v) if v.as_i64() == Some(0) => return Ok(None), None | Some(Value::Bool(true)) => (), Some(v) if v.as_i64() == Some(1) => (), _ => return Err(fail("Invalid enable flag")) }
    let port = obj.get("server_port").or_else(|| obj.get("port")).and_then(Value::as_u64).filter(|p| *p > 0 && *p <= 65535).ok_or_else(|| fail("Invalid server_port"))?;
    if obj.get("port").is_some_and(|v| v.as_u64() != Some(port)) { return Err(fail("Conflicting port fields")); }
    let string = |key| obj.get(key).and_then(Value::as_str).ok_or_else(|| fail("Missing node field"));
    let host = string("hostname")?; let method = string("encryption")?; let password = string("password")?;
    let fallback = format!("{}:{}", host, port);
    let tag = obj.get("alias").and_then(Value::as_str).filter(|v| !v.is_empty()).unwrap_or(&fallback);
    if [host,method,password,tag].iter().any(|v| !clean(v)) { return Err(fail("Missing or unsafe node field")); }
    parse_line(&format!("shadowsocks={}:{}, method={}, password={}, fast-open=false, udp-relay=false, tag={}",host,port,method,password,tag), domain).map(Some)
}
pub struct Report { pub nodes: Vec<Node>, pub rows: Vec<(String, bool, String)>, pub failures: usize }
pub fn parse_input(text: &str, domain: Option<&str>) -> Result<Report> {
    let text = text.trim_start_matches('\u{feff}').trim();
    let mut report = Report { nodes: Vec::new(), rows: Vec::new(), failures: 0 };
    let mut seen = HashSet::new();
    let mut add = |index: usize, result: Result<Option<Node>>| {
        match result {
            Ok(Some(n)) if seen.insert(n.tag().to_string()) => { report.rows.push((n.tag().into(),true,"accepted".into())); report.nodes.push(n); },
            Ok(Some(_)) => { report.failures += 1; report.rows.push((format!("item {}",index),false,"duplicate tag in input".into())); },
            Ok(None) => report.rows.push((format!("item {}",index),true,"disabled; skipped".into())),
            Err(e) => { report.failures += 1; report.rows.push((format!("item {}",index),false,e.to_string())); }
        }
    };
    if text.starts_with('[') || text.starts_with('{') {
        let value: Value = serde_json::from_str(text).map_err(|_| fail("Malformed JSON; no private input was logged"))?;
        let array = value.as_array().ok_or_else(|| fail("Expected a servers JSON array"))?;
        if domain.is_some() && array.len() != 1 { return Err(fail("--domain requires exactly one input node")); }
        for (i,v) in array.iter().enumerate() { add(i+1, json_node(v,domain)); }
    } else {
        let lines = text.lines().enumerate().filter(|(_,s)| !s.trim().is_empty() && !s.trim_start().starts_with(['#',';'])).collect::<Vec<_>>();
        if domain.is_some() && lines.len() != 1 { return Err(fail("--domain requires exactly one input node")); }
        for (i,line) in lines { add(i+1,parse_line(line.trim(),domain).map(Some)); }
    }
    if report.rows.is_empty() { return Err(fail("No nodes in input")); }
    Ok(report)
}
pub fn merge(existing: &[Node], incoming: &[Node]) -> Vec<Node> {
    let mut out = existing.to_vec(); let mut positions: HashMap<String,usize> = out.iter().enumerate().map(|(i,n)|(n.tag().into(),i)).collect();
    for n in incoming {
        let identity=|a:&Node,b:&Node| match (&a.relay,&b.relay) {
            (Some(a),Some(b))=>{
                let p=|v:&Value|v["protocol"].as_str().unwrap_or("").to_string();
                a["id"].as_u64().is_some() && a["id"]==b["id"] && p(a)==p(b)
            },_=>false
        };
        if let Some(i)=out.iter().position(|old|identity(old,n)) {
            let mut updated=n.clone();updated.port=out[i].port;
            for key in ["username","password","tag"] {
                if let Some((_,old))=out[i].options.iter().find(|(k,_)|k==key) {
                    if let Some((_,new))=updated.options.iter_mut().find(|(k,_)|k==key) {*new=old.clone();}
                }
            }
            out[i]=updated;
        } else if let Some(i)=positions.get(n.tag()) {out[*i]=n.clone();} else {positions.insert(n.tag().into(),out.len());out.push(n.clone());}
    }
    out
}
#[cfg(test)] mod tests {
    use super::*;
    const LINE: &str = "shadowsocks=relay.example.com:8443, method=chacha20-ietf-poly1305, password=fixture-only, tag=US1";
    #[test] fn explicit_protocol_delivery_and_stable_reimport() {
        let key=STANDARD.encode([9u8;16]);
        let base=serde_json::json!({"id":51,"protocol":"ss2022","delivery":"native","host":"node.example.com","port":443,"method":"2022-blake3-aes-128-gcm","password":format!("{}:{}",key,key),"alias":"Standard"});
        let native=json_node(&base,None).unwrap().unwrap();assert!(native.relay.is_none());assert!(native.render("8.8.8.8").starts_with("shadowsocks="));
        let mut relay=base.clone();relay["delivery"]="relay".into();let first=json_node(&relay,None).unwrap().unwrap();assert_eq!(first.relay.as_ref().unwrap()["protocol"],"ss2022");
        let mut changed=relay.clone();changed["alias"]="Renamed".into();changed["host"]="changed.example.com".into();
        let next=json_node(&changed,None).unwrap().unwrap();let merged=merge(&[first.clone()],&[next]);assert_eq!(merged.len(),1);assert_eq!(merged[0].port,first.port);assert_eq!(merged[0].options,first.options);
        let mut bad=base.clone();bad["password"]="not-base64".into();assert!(json_node(&bad,None).is_err());
        bad=base.clone();bad["protocol"]="papaya".into();assert!(json_node(&bad,None).is_err());
        bad=base.clone();bad["protocol"]="unknown".into();assert!(json_node(&bad,None).is_err());
    }
    #[test] fn relay_migration_and_merge() {
        let old=parse_line(LINE,None).unwrap();
        let legacy=serde_json::json!({"host":old.host,"port":old.port,"options":old.options});
        let decoded:Node=serde_json::from_value(legacy).unwrap();assert!(decoded.relay.is_none());
        let key=STANDARD.encode([7u8;16]);
        let value=serde_json::json!([{"id":42,"protocol":"papaya","host":"relay.example.com","port":443,"params":{},"auth":{"server_key":key,"user_key":key},"alias":"Test"}]);
        let a=parse_input(&value.to_string(),None).unwrap().nodes.remove(0);
        let b=parse_input(&value.to_string(),None).unwrap().nodes.remove(0);
        assert_eq!(a.port,b.port);assert!(a.render("127.0.0.1").starts_with("socks5="));
        let merged=merge(&[decoded.clone()],&[a]);assert_eq!(merged.len(),2);assert!(merged[0]==decoded);
        let mut bad=value.clone();bad[0]["params"]=serde_json::json!({"ascii-first-request":false});assert_eq!(parse_input(&bad.to_string(),None).unwrap().failures,1);
    }
    #[test] fn partial_and_merge() { let r=parse_input(&format!("{}\nbad\n{}",LINE,LINE),None).unwrap(); assert_eq!(r.nodes.len(),1); assert_eq!(r.failures,2); let a=parse_line(LINE,None).unwrap(); let b=parse_line(&LINE.replace("8443","8444"),None).unwrap(); assert!(merge(&[a],&[b.clone()])==vec![b]); }
    #[test] fn ss_and_json() { let uri=format!("ss://{}@example.com:443#US1",STANDARD.encode("chacha20-ietf-poly1305:fixture-only")); assert_eq!(parse_line(&uri,None).unwrap().tag(),"US1"); let r=parse_input(r#"[{"hostname":"example.com","server_port":8443,"port":8443,"encryption":"chacha20-ietf-poly1305","password":"fixture-only","alias":"JP1","enable":1},{"enable":0}]"#,None).unwrap(); assert_eq!(r.nodes.len(),1); assert_eq!(r.failures,0); }
    #[test] fn json_ignores_provider_metadata() {
        let input=r#"[{"hostname":"relay.example.com","server_port":8443,"encryption":"chacha20-ietf-poly1305","password":"fixture-only","alias":"Example","enable":1}]"#;
        let expected=parse_input(input,None).unwrap();
        for metadata in [serde_json::json!(73),serde_json::json!("provider-defined"),serde_json::Value::Null] {
            let mut value:Value=serde_json::from_str(input).unwrap();
            value[0]["category"]=metadata;
            let report=parse_input(&value.to_string(),None).unwrap();
            assert_eq!(report.failures,0);assert!(report.nodes==expected.nodes);
        }
        let incomplete=r#"[{"hostname":"relay.example.com","server_port":8443,"category":73}]"#;
        let report=parse_input(incomplete,None).unwrap();assert_eq!(report.failures,1);assert!(report.nodes.is_empty());
    }
    #[test] fn no_injection_or_private_ip() { for s in [LINE.replace("relay.example.com","127.0.0.1"),LINE.replace("US1","US1\nsecret"),LINE.replace("fixture-only","<redacted>")] { assert!(parse_line(&s,None).is_err()); } }
}
