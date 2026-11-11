mod dns;
mod nodes;
mod service;
mod store;
mod upgrade;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueHint};
use std::{io::{self, IsTerminal, Read, Write}, path::{Path, PathBuf}, process::{Command, Stdio}};
use store::{Config, Lock};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn fail(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> { Box::new(io::Error::other(message.into())) }
const VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASE_DATE: &str = "2026-11-11";
const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
fn version_text() -> String { format!("dohsub version {} ({})\n{}\n",VERSION,RELEASE_DATE,REPOSITORY) }

#[derive(Parser)]
#[command(name="dohsub", about="Serve a local QX subscription with automatically updated DoH-resolved IPv4 addresses.", disable_version_flag=true,
    after_help="Examples:\n  dohsub config set --doh https://dns.example.com/dns-query\n  dohsub import --file ~/Downloads/nodes.txt\n  dohsub start\n  dohsub copy-resource\n\nmacOS and Linux. curl is required for HTTPS. State: ~/.dohsub (private).")]
struct Cli {
    #[arg(long, short='V', global=true, help="Print version and repository (no network)")]
    version: bool,
    #[command(subcommand)] command: Option<Action>,
}
#[derive(Args)]
struct Connection {
    #[arg(long, value_hint=ValueHint::Other, help="HTTPS DNS-wire DoH URL; otherwise use cached configuration")]
    doh: Option<String>,
    #[arg(long, value_parser=clap::value_parser!(u16).range(1..), value_hint=ValueHint::Other, help="Local port; explicit collision fails, cached/default collision increments (default 55355)")]
    port: Option<u16>,
    #[arg(long, value_hint=ValueHint::Other, help="Bind outgoing DoH requests to this interface (e.g. en0); cached if omitted")]
    interface: Option<String>,
}
#[derive(Subcommand)]
enum Action {
    #[command(about="Save or inspect private cached settings; changes apply on next start/restart", args_conflicts_with_subcommands=true, after_help="Examples:\n  dohsub config set --doh https://dns.example.com/dns-query --port 55355\n  dohsub config --list\n\nConfiguration files are private. --list redacts URL query values. --clear-interface restores automatic routing.")]
    Config { #[arg(long)] list: bool, #[command(subcommand)] action: Option<ConfigAction> },
    #[command(about="Validate DoH, reserve a loopback port and start the background subscription service", after_help="Examples:\n  dohsub start\n  dohsub start --doh https://dns.example.com/dns-query --port 55356 --import nodes.json\n\nRequires curl and a reachable HTTPS DNS-wire endpoint. A valid negative DNS reply proves reachability. Optional import is saved before startup; failed startup leaves those nodes staged. Prints a private subscription URL. Does not enable login startup.")]
    Start { #[command(flatten)] connection: Connection, #[arg(long, value_hint=ValueHint::FilePath, help="Import a private node file before starting")] import: Option<PathBuf> },
    #[command(about="Stage nodes from a private QX text, ss:// or servers JSON file", after_help="Examples:\n  dohsub import --file ~/Downloads/nodes.txt\n  dohsub import --file ~/Downloads/servers.json -y\n\nFile must be owned by you and mode 600. Same tags update existing nodes; other nodes remain. Partial success saves accepted items and exits 1. In a running service, only y/yes or -y loads changes; EOF/non-interactive input leaves them staged. Acceptance validates configuration, not proxy connectivity.")]
    Import { #[arg(long, value_hint=ValueHint::FilePath)] file: PathBuf, #[arg(long, value_hint=ValueHint::Other, help="Replace the host; requires exactly one input node")] domain: Option<String>, #[arg(short, long, help="Load staged nodes immediately if running")] yes: bool },
    #[command(about="Import node configuration from the local clipboard", after_help="Example:\n  dohsub add --domain relay.example.com -y\n\nRequires pbpaste/pbcopy on macOS; wl-paste/wl-copy or xclip on Linux. Accepts full QX nodes, ss:// links or servers JSON, not just a hostname. Clears the clipboard only after all items are accepted. Same staging/confirmation behavior as import.")]
    Add { #[arg(long, value_hint=ValueHint::Other)] domain: Option<String>, #[arg(short, long)] yes: bool },
    #[command(about="Stop then start using cached configuration; validates DoH before stopping", after_help="Example: dohsub restart\nA configuration change takes effect here. Subscription clients must refresh if the local port changes.")]
    Restart,
    #[command(about="Stop the managed dohsub daemon", after_help="Example: dohsub stop\nWaits for an in-progress HTTPS request (up to 8 seconds). Already stopped is successful.")]
    Stop,
    #[command(about="List staged nodes and active publication status without passwords", after_help="Example: dohsub list\nPublished means a public IPv4 is available, not that the Shadowsocks handshake has been tested.")]
    List,
    #[command(about="Load staged nodes and queue one immediate DNS refresh, bypassing backoff once", after_help="Example: dohsub refresh\nRequires a running daemon. Returns after queuing; use list to inspect results. Does not force QX to fetch its subscription.")]
    Refresh,
    #[command(about="Remove one node by exact tag, with a one-level backup; reload if running", after_help="Example: dohsub remove 'Example-US'\nUse the original tag without the generated · AutoIP suffix. restore can undo the latest node modification.")]
    Remove { #[arg(value_hint=ValueHint::Other)] name: String },
    #[command(about="Copy the private QX server_remote subscription line to the clipboard", after_help="Example: dohsub copy-resource\nRequires a running service. Paste into QX [server_remote], save and manually refresh. The URL contains an access token; keep it private.")]
    CopyResource,
    #[command(about="Restore the node snapshot from before the latest modification; reload if running", after_help="Example: dohsub restore\nSwaps current and previous snapshots, so restore can also undo itself. Does not restore settings or DNS cache.")]
    Restore,
    #[command(about="Print exactly two lines: version and repository URL")]
    Version,
    #[command(about="Check GitHub releases and optionally upgrade the owning installation", after_help="Example: dohsub upgrade\nNetwork is required. Stable releases only. Confirmation defaults to no; non-interactive updates require --yes. Homebrew uses its own updater; standalone installations require the install.sh ownership marker. Missing or invalid release metadata reports a failed check, not 'latest'.")]
    Upgrade { #[arg(short, long)] yes: bool },
    #[command(about="Generate shell completion without reading or writing configuration", after_help="Examples:\n  dohsub completion zsh > _dohsub\n  dohsub completion bash > dohsub.bash\n  dohsub completion fish > dohsub.fish")]
    Completion { shell: clap_complete::Shell },
    #[command(about="Write the man page to stdout without initializing state")]
    Man,
    #[command(name="__serve", hide=true)] Serve { #[arg(long)] fd: i32, #[arg(long)] lock_fd: i32 },
}
#[derive(Subcommand)]
enum ConfigAction { #[command(about="Persist supplied fields; existing omitted fields are retained", after_help="Example: dohsub config set --doh https://dns.example.com/dns-query --port 55355\nValidates URL syntax only; start checks network reachability.")] Set { #[command(flatten)] connection: Connection, #[arg(long, conflicts_with="interface")] clear_interface: bool } }
fn apply(mut config: Config, c: Connection) -> Result<Config> {
    if let Some(doh)=c.doh {dns::validate_url(&doh)?;config.doh=Some(doh);}
    if let Some(port)=c.port {config.port=port;}
    if let Some(interface)=c.interface {if interface.is_empty() || interface.len()>64 || !interface.bytes().all(|b|b.is_ascii_alphanumeric() || b"._:-".contains(&b)) {return Err(fail("Invalid interface name"));}config.interface=Some(interface);}
    Ok(config)
}
fn expand(path: &Path) -> PathBuf {if let Ok(s)=path.strip_prefix("~") {if let Some(h)=std::env::var_os("HOME") {return PathBuf::from(h).join(s);}}path.to_path_buf()}
fn load_nodes(dir: &Path) -> Result<Vec<nodes::Node>> {store::load(&dir.join("nodes.json"))}
fn change_nodes(dir: &Path, nodes: &[nodes::Node]) -> Result<()> {
    let old=load_nodes(dir)?;
    store::save(&dir.join("nodes.previous.json"),&old)?;
    store::save(&dir.join("nodes.json"),&nodes)
}
fn confirm(prompt: &str) -> Result<bool> {
    if !io::stdin().is_terminal() {return Ok(false);}
    print!("{} [y/N] ",prompt);io::stdout().flush()?;
    let mut line=String::new();io::stdin().read_line(&mut line)?;
    Ok(matches!(line.trim().to_ascii_lowercase().as_str(),"y"|"yes"))
}
fn clipboard(copy: Option<&str>) -> Result<String> {
    #[cfg(target_os="macos")] let candidates:Vec<(&str,Vec<&str>)>=if copy.is_some() {vec![("pbcopy",vec![])]} else {vec![("pbpaste",vec![])]};
    #[cfg(not(target_os="macos"))] let candidates:Vec<(&str,Vec<&str>)>=if copy.is_some() {vec![("wl-copy",vec![]),("xclip",vec!["-selection","clipboard"])]} else {vec![("wl-paste",vec!["--no-newline"]),("xclip",vec!["-selection","clipboard","-o"])]};
    for (program,args) in candidates {
        let Ok(mut child)=Command::new(program).args(args).stdin(if copy.is_some(){Stdio::piped()}else{Stdio::null()}).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {continue;};
        if let Some(text)=copy {let mut input=child.stdin.take().ok_or_else(||fail("Cannot write clipboard"))?;input.write_all(text.as_bytes())?;drop(input);}
        let mut bytes=Vec::new();child.stdout.take().unwrap().take(1_048_577).read_to_end(&mut bytes)?;
        if bytes.len()>1_048_576 {let _=child.kill();let _=child.wait();return Err(fail("Clipboard exceeds 1 MiB"));}
        if child.wait()?.success() {return String::from_utf8(bytes).map_err(|_|fail("Clipboard is not UTF-8"));}
    }
    Err(fail("Clipboard unavailable; use a local desktop session or import --file"))
}
fn import_text(dir: &Path, text: &str, domain: Option<&str>, yes: bool, clear_clipboard: bool) -> Result<()> {
    let report=nodes::parse_input(text,domain)?;
    let _guard=Lock::acquire(&dir.join("nodes.lock"),true)?;
    let old=load_nodes(dir)?;
    let merged=nodes::merge(&old,&report.nodes);
    if !report.nodes.is_empty() {change_nodes(dir,&merged)?;}
    let color=io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    for (label,ok,detail) in &report.rows {
        let action=if *ok && detail=="accepted" {if old.iter().any(|n|n.tag()==label){"updated"}else{"added"}} else {detail};
        if color {println!("\x1b[{}m{}\x1b[0m {}: {}",if *ok{32}else{31},if *ok{"✓"}else{"✗"},label,action);} else {println!("{} {}: {}",if *ok{"✓"}else{"✗"},label,action);}
    }
    println!("Accepted {} node(s), failed {}; {} staged in total.",report.nodes.len(),report.failures,merged.len());
    if report.nodes.is_empty() {return Err(fail("No nodes imported; existing configuration unchanged"));}
    if clear_clipboard && report.failures==0 && clipboard(Some("")).is_err() {eprintln!("Imported, but could not clear clipboard");}
    if service::running(dir) {
        if yes || confirm("Load staged nodes into the running service now?")? {let value=service::request(dir,service::Request::Reload)?;println!("Loaded {} node(s). Refresh the subscription in QX.",value["loaded_nodes"]);}
        else {println!("Saved only. Run dohsub refresh to load later.");}
    } else {println!("Saved. Start dohsub to publish these nodes.");}
    if report.failures>0 {return Err(fail("Partial import: accepted nodes were saved; rejected items were not"));}
    Ok(())
}
fn started(dir: &Path, config: Config, explicit: bool) -> Result<()> {
    let port=service::start(dir,config,explicit)?;
    println!("dohsub started on 127.0.0.1:{}\nSubscription (private): http://127.0.0.1:{}/sub/{}/nodes.conf",port,port,store::token(dir)?);
    println!("Use dohsub copy-resource to copy the QX subscription line.");Ok(())
}
fn completion(shell: clap_complete::Shell) -> Result<()> {
    let mut cmd=Cli::command();let mut out=Vec::new();clap_complete::generate(shell,&mut cmd,"dohsub",&mut out);
    if shell==clap_complete::Shell::Bash {
        let script=String::from_utf8(out)?.replacen("_dohsub() {","_dohsub_generated() {",1);
        let script=script.replace("-o bashdefault -o default ","");
        let point=script.find("if [[ \"${BASH_VERSINFO[0]}\" -eq 4").ok_or_else(||fail("Completion generator contract changed"))?;
        let (body,register)=script.split_at(point);print!("{}{}{}",body,include_str!("../docs/bash-wrapper.sh"),register);
    } else {io::stdout().write_all(&out)?;}
    Ok(())
}
fn run() -> Result<()> {
    let cli=Cli::parse();
    if cli.version {print!("{}",version_text());return Ok(());}
    let Some(action)=cli.command else {Cli::command().print_help()?;println!();return Ok(());};
    match action {
        Action::Version=>{print!("{}",version_text());return Ok(());},
        Action::Completion{shell}=>{completion(shell)?;return Ok(());},
        Action::Man=>{let mut cmd=Cli::command();cmd=cmd.version(VERSION);clap_mangen::Man::new(cmd).render(&mut io::stdout())?;print!("{}",include_str!("../man/extra.roff"));return Ok(());},
        Action::Upgrade{yes}=>return upgrade::run(yes),
        _=>()
    }
    let dir=store::home()?;store::init(&dir)?;
    match action {
        Action::Config{list,action}=>match action {
            Some(ConfigAction::Set{connection,clear_interface})=>{let _guard=Lock::acquire(&dir.join("config.lock"),true)?;let mut c=apply(store::load(&dir.join("config.json"))?,connection)?;if clear_interface{c.interface=None;}store::save(&dir.join("config.json"),&c)?;println!("Configuration saved. Run start or restart to apply.");},
            None if list=>{let mut c:Config=store::load(&dir.join("config.json"))?;if let Some(s)=&c.doh {let mut u=url::Url::parse(s)?;if u.query().is_some(){u.set_query(Some("REDACTED"));}c.doh=Some(u.to_string());}println!("{}",serde_json::to_string_pretty(&c)?);},
            _=>return Err(fail("Use dohsub config --list or dohsub config set --help"))
        },
        Action::Start{connection,import}=>{
            let _config_guard=Lock::acquire(&dir.join("config.lock"),true)?;
            let explicit=connection.port.is_some();let c=apply(store::load(&dir.join("config.json"))?,connection)?;
            if service::running(&dir) {return Err(fail("dohsub is already running; use restart or refresh"));}
            if c.doh.is_none(){return Err(fail("DoH is not configured; run dohsub config set --doh HTTPS_URL"));}
            if let Some(file)=import {let data=store::read(&expand(&file))?;let text=std::str::from_utf8(&data).map_err(|_|fail("Node file is not UTF-8"))?;import_text(&dir,text,None,true,false)?;}
            started(&dir,c,explicit)?;
        },
        Action::Import{file,domain,yes}=>{let bytes=store::read(&expand(&file))?;let text=std::str::from_utf8(&bytes).map_err(|_|fail("Node file is not UTF-8"))?;import_text(&dir,text,domain.as_deref(),yes,false)?;},
        Action::Add{domain,yes}=>{let text=clipboard(None)?;import_text(&dir,&text,domain.as_deref(),yes,true)?;},
        Action::Restart=>{let _config_guard=Lock::acquire(&dir.join("config.lock"),true)?;let c:Config=store::load(&dir.join("config.json"))?;let nodes=load_nodes(&dir)?;dns::preflight(&c,nodes.iter().find(|n|!nodes::public_ip(&n.host)).map(|n|n.host.as_str()).unwrap_or("example.com"))?;if service::running(&dir){service::stop(&dir)?;}started(&dir,c,false)?;},
        Action::Stop=>{if service::running(&dir){service::stop(&dir)?;println!("dohsub stopped.");}else{println!("dohsub is already stopped.");}},
        Action::List=>{let nodes=load_nodes(&dir)?;let active=service::request(&dir,service::Request::Status).ok();for n in &nodes {let published=active.as_ref().and_then(|s|s["nodes"].as_array()).is_some_and(|ns|ns.iter().any(|v|v["tag"]==n.tag() && v["published"]==true));println!("{}\t{}:{}\t{}",n.tag(),n.host,n.port,if published{"published (active tag)"}else{"staged / unresolved"});}println!("{} staged node(s). Service: {}",nodes.len(),active.as_ref().and_then(|v|v["status"].as_str()).unwrap_or("stopped"));if let Some(value)=active {println!("Active: {} nodes; {} published. DNS retries: {}",value["node_count"],value["published_node_count"],value["dns_retries"]);}},
        Action::Refresh=>{let v=service::request(&dir,service::Request::Refresh)?;println!("Loaded {} node(s); one DNS refresh queued. Use list to inspect results, then refresh QX.",v["loaded_nodes"]);},
        Action::Remove{name}=>{let _guard=Lock::acquire(&dir.join("nodes.lock"),true)?;let old=load_nodes(&dir)?;let new=old.iter().filter(|n|n.tag()!=name).cloned().collect::<Vec<_>>();if old.len()==new.len(){return Err(fail("No node with that exact tag; use dohsub list"));}change_nodes(&dir,&new)?;if service::running(&dir){service::request(&dir,service::Request::Reload)?;}println!("Removed node. Use restore to undo.");},
        Action::Restore=>{let _guard=Lock::acquire(&dir.join("nodes.lock"),true)?;let path=dir.join("nodes.previous.json");if !path.exists(){return Err(fail("No previous node snapshot"));}let previous:Vec<nodes::Node>=store::load(&path)?;change_nodes(&dir,&previous)?;if service::running(&dir){service::request(&dir,service::Request::Reload)?;}println!("Restored {} node(s).",previous.len());},
        Action::CopyResource=>{let active=service::request(&dir,service::Request::Status)?;let port=active["port"].as_u64().ok_or_else(||fail("Invalid service port"))?;let text=format!("http://127.0.0.1:{}/sub/{}/nodes.conf, tag=dohsub, update-interval=60, opt-parser=false, enabled=true",port,store::token(&dir)?);clipboard(Some(&text))?;println!("Private QX subscription line copied. Paste into [server_remote] and refresh.");},
        Action::Serve{fd,lock_fd}=>unsafe {service::serve(&dir,fd,lock_fd)?;},
        _=>unreachable!()
    }
    Ok(())
}
fn main() {if let Err(error)=run(){eprintln!("Error: {}",error);std::process::exit(1);}}

#[cfg(test)] mod cli_tests { use super::*; #[test] fn cli_contract_is_valid(){Cli::command().debug_assert();} }
