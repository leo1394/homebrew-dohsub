use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hickory_proto::{op::{Message, Query, ResponseCode}, rr::{Name, RData, RecordType}};
use std::{collections::HashSet, io::Write, process::{Command, Stdio}};
use url::Url;
use crate::{Result, fail, store::Config, nodes::public_ip};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Reason { Negative, Transient }
pub struct Failure { pub reason: Reason }
pub fn validate_url(value: &str) -> Result<()> {
    let u = Url::parse(value).map_err(|_| fail("Invalid DoH URL"))?;
    if u.scheme() != "https" || u.host_str().is_none() || !u.username().is_empty() || u.password().is_some() || u.fragment().is_some() { return Err(fail("DoH must be an HTTPS URL without userinfo or fragments")); }
    if value.chars().any(|c| c.is_control() || c == '"' || c == '\\') { return Err(fail("Unsafe DoH URL")); }
    Ok(())
}
pub fn query(config: &Config, host: &str) -> Result<Message> {
    let doh = config.doh.as_deref().ok_or_else(|| fail("DoH is not configured; run dohsub config set --doh HTTPS_URL"))?;
    validate_url(doh)?;
    let name = Name::from_ascii(host).map_err(|_| fail("Invalid DNS name"))?;
    let mut request = Message::new(); request.set_id(0).set_recursion_desired(true).add_query(Query::query(name.clone(), RecordType::A));
    let packet = request.to_vec().map_err(|_| fail("Cannot encode DNS request"))?;
    let mut url = Url::parse(doh)?;
    let pairs = url.query_pairs().filter(|(k,_)| k != "dns").map(|(k,v)|(k.into_owned(),v.into_owned())).collect::<Vec<_>>();
    url.set_query(None);
    for (k,v) in pairs { url.query_pairs_mut().append_pair(&k,&v); }
    url.query_pairs_mut().append_pair("dns", &URL_SAFE_NO_PAD.encode(packet));
    let mut cmd = Command::new("curl");
    cmd.args(["--disable","--config","-","--silent","--show-error","--fail","--noproxy","*","--connect-timeout","3","--max-time","8","--max-filesize","65535","--header","accept: application/dns-message"]);
    if let Some(interface) = &config.interface { cmd.arg("--interface").arg(interface); }
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_| fail("curl is required for HTTPS requests"))?;
    let mut input = child.stdin.take().ok_or_else(|| fail("Cannot prepare DoH request"))?;
    let input_result = writeln!(input,"url = \"{}\"",url.as_str()); drop(input);
    let result = child.wait_with_output()?;
    input_result?;
    if !result.status.success() { return Err(fail("DoH HTTPS request failed (timeout, TLS, HTTP or network); URL credentials were not logged")); }
    parse_response(&result.stdout, &name)
}
pub fn parse_response(bytes: &[u8], name: &Name) -> Result<Message> {
    let answer = Message::from_vec(bytes).map_err(|_| fail("DoH returned malformed DNS data"))?;
    if answer.id() != 0 || answer.message_type() != hickory_proto::op::MessageType::Response || answer.truncated() || answer.op_code() != hickory_proto::op::OpCode::Query || answer.queries().len()!=1 || answer.queries()[0].name()!=name || answer.queries()[0].query_type()!=RecordType::A || answer.queries()[0].query_class()!=hickory_proto::rr::DNSClass::IN { return Err(fail("DoH returned a mismatched DNS response")); }
    Ok(answer)
}
pub fn addresses(answer: &Message, host: &str) -> std::result::Result<(Vec<String>,u64), Failure> {
    let error = |reason| Failure {reason};
    if answer.response_code() != ResponseCode::NoError { return Err(error(if answer.response_code()==ResponseCode::NXDomain {Reason::Negative} else {Reason::Transient})); }
    let mut names = HashSet::from([host.trim_end_matches('.').to_lowercase()]);
    let mut ttl = 3600u64;
    for _ in 0..answer.answers().len() {
        for rr in answer.answers() {
            if rr.dns_class()!=hickory_proto::rr::DNSClass::IN || !names.contains(&rr.name().to_ascii().trim_end_matches('.').to_lowercase()) { continue; }
            if let Some(RData::CNAME(target)) = rr.data() { names.insert(target.to_ascii().trim_end_matches('.').to_lowercase()); ttl=ttl.min(rr.ttl() as u64); }
        }
    }
    let mut values = Vec::new();
    for rr in answer.answers() {
        if rr.dns_class()!=hickory_proto::rr::DNSClass::IN || !names.contains(&rr.name().to_ascii().trim_end_matches('.').to_lowercase()) { continue; }
        if let Some(RData::A(ip))=rr.data() { let ip=ip.to_string(); if public_ip(&ip) { ttl=ttl.min(rr.ttl() as u64); if !values.contains(&ip) { values.push(ip); } } }
    }
    if values.is_empty() { return Err(error(Reason::Negative)); }
    Ok((values,ttl.clamp(5,3600)))
}
pub fn resolve(config: &Config, host: &str) -> std::result::Result<(Vec<String>,u64), Failure> {
    match query(config,host) { Ok(answer)=>addresses(&answer,host), Err(_)=>Err(Failure {reason:Reason::Transient}) }
}
pub fn preflight(config: &Config, host: &str) -> Result<()> {
    query(config,host)?; // A valid NXDOMAIN/REFUSED response still proves the endpoint is reachable.
    Ok(())
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn validates_https_and_queries() {
        assert!(validate_url("https://192.0.2.53/dns-query").is_ok());
        for s in ["http://localhost/query","https://user:secret@example.com/dns","file:///tmp/a"] { assert!(validate_url(s).is_err()); }
        let name=Name::from_ascii("example.com").unwrap(); let mut msg=Message::new();
        msg.set_message_type(hickory_proto::op::MessageType::Response).add_query(Query::query(name.clone(),RecordType::A)).set_response_code(ResponseCode::NXDomain);
        let decoded=parse_response(&msg.to_vec().unwrap(),&name).unwrap();
        assert!(addresses(&decoded,"example.com").is_err_and(|e| e.reason==Reason::Negative));
        assert!(parse_response(&msg.to_vec().unwrap(),&Name::from_ascii("other.com").unwrap()).is_err());
    }
}
