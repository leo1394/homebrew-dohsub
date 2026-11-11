use std::{fs, io::{self, IsTerminal, Write}, path::PathBuf, process::Command};
use serde_json::Value;
use sha2::{Digest, Sha256};
use crate::{Result, fail, VERSION, REPOSITORY};

#[derive(Debug, PartialEq)]
enum Channel { Current, Newer, Ahead }
fn compare(current: &str, latest: &str) -> Result<Channel> {
    let current=semver::Version::parse(current).map_err(|_|fail("Invalid installed version"))?;
    let latest=semver::Version::parse(latest.trim_start_matches('v')).map_err(|_|fail("Invalid release version"))?;
    if !latest.pre.is_empty() {return Err(fail("Release metadata unexpectedly selected a prerelease"));}
    Ok(match current.cmp_precedence(&latest) {std::cmp::Ordering::Less=>Channel::Newer,std::cmp::Ordering::Greater=>Channel::Ahead,std::cmp::Ordering::Equal=>Channel::Current})
}
fn fetch(url: &str) -> Result<Vec<u8>> {
    let out=Command::new("curl").args(["--disable","--fail","--silent","--show-error","--location","--proto","=https","--proto-redir","=https","--connect-timeout","5","--max-time","120","--max-filesize","33554432",url]).output().map_err(|_|fail("curl is required to check releases"))?;
    if !out.status.success(){return Err(fail("Release check/download failed; repository or release may not be published yet"));}
    Ok(out.stdout)
}
fn asset_url<'a>(release: &'a Value,name:&str,tag:&str)->Result<&'a str> {
    let assets=release["assets"].as_array().ok_or_else(||fail("Release assets are missing"))?;
    let url=assets.iter().find(|a|a["name"]==name).and_then(|a|a["browser_download_url"].as_str()).ok_or_else(||fail("No release asset for this platform"))?;
    let prefix=format!("{}/releases/download/{}/",REPOSITORY,tag);
    if url!=format!("{}{}",prefix,name) {return Err(fail("Release asset URL does not match this repository and tag"));}
    Ok(url)
}
pub fn run(yes:bool)->Result<()> {
    let release:Value=serde_json::from_slice(&fetch("https://api.github.com/repos/leo1394/homebrew-dohsub/releases/latest")?).map_err(|_|fail("Invalid GitHub release response"))?;
    if release["draft"]!=false || release["prerelease"]!=false {return Err(fail("No eligible stable release"));}
    let tag=release["tag_name"].as_str().ok_or_else(||fail("Missing release tag"))?;
    match compare(VERSION,tag)? {
        Channel::Current=>{println!("You're already on the latest version of dohsub ({}).",tag);return Ok(());},
        Channel::Ahead=>{println!("Installed v{} is newer than the stable release {}; no downgrade performed.",VERSION,tag);return Ok(());},
        Channel::Newer=>println!("{} is out! You're currently on v{}.",tag,VERSION)
    }
    let exe=fs::canonicalize(std::env::current_exe()?)?;
    let text=exe.to_string_lossy();
    let brew=text.contains("/Cellar/dohsub/");
    let prefix=exe.parent().and_then(|p|p.parent()).ok_or_else(||fail("Unsupported installation layout"))?;
    let marker=prefix.join("share/dohsub/install.json");
    let mut destination=exe.clone();
    if !brew && !marker.is_file(){return Err(fail("Unsupported installation; use Homebrew or install.sh before upgrading"));}
    if brew {
        let out=Command::new("brew").args(["--prefix","dohsub"]).output()?;
        let root=PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        if !out.status.success() || !exe.starts_with(fs::canonicalize(root)?) {return Err(fail("Homebrew does not own this executable"));}
        if !Command::new("brew").arg("update").status()?.success(){return Err(fail("Homebrew metadata update failed"));}
        let info=Command::new("brew").args(["info","--json=v2","leo1394/dohsub/dohsub"]).output()?;
        let info:Value=if info.status.success(){serde_json::from_slice(&info.stdout).map_err(|_|fail("Invalid Homebrew metadata"))?}else{return Err(fail("Cannot read Homebrew formula metadata"));};
        let formula=&info["formulae"][0];
        if formula["full_name"]!="leo1394/dohsub/dohsub" || formula["versions"]["stable"].as_str()!=Some(tag.trim_start_matches('v')) {return Err(fail("This upstream release is not yet available through the owning Homebrew formula; retry later"));}
        println!("Method: Homebrew (leo1394/dohsub/dohsub), target {}",tag);
    } else {
        let v:Value=serde_json::from_slice(&fs::read(&marker)?).map_err(|_|fail("Invalid standalone installation marker"))?;
        if v["repository"]!=REPOSITORY {return Err(fail("Standalone installation marker belongs to a different repository"));}
        println!("Method: standalone, target {}",tag);
    }
    if !yes {
        if !io::stdin().is_terminal(){return Err(fail("Update available; rerun interactively or with upgrade --yes"));}
        if !crate::confirm("Do you want to proceed with the upgrade?")? {println!("Upgrade cancelled.");return Ok(());}
    }
    if brew {
        let info=Command::new("brew").args(["info","--json=v2","leo1394/dohsub/dohsub"]).output()?;
        let info:Value=serde_json::from_slice(&info.stdout).map_err(|_|fail("Cannot revalidate Homebrew target"))?;
        if info["formulae"][0]["versions"]["stable"].as_str().is_none_or(|v|v!=tag.trim_start_matches('v')) {return Err(fail("Homebrew target changed after confirmation; rerun upgrade"));}
        if !Command::new("brew").env("HOMEBREW_NO_AUTO_UPDATE","1").args(["upgrade","leo1394/dohsub/dohsub"]).status()?.success(){return Err(fail("Homebrew upgrade failed; check brew diagnostics"));}
        let out=Command::new("brew").args(["--prefix","dohsub"]).output()?;
        if !out.status.success(){return Err(fail("Cannot locate upgraded Homebrew binary"));}
        destination=PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()).join("bin/dohsub");
    } else {
        let platform=match (std::env::consts::OS,std::env::consts::ARCH) {("macos","aarch64")=>"aarch64-apple-darwin",("macos","x86_64")=>"x86_64-apple-darwin",("linux","aarch64")=>"aarch64-unknown-linux-gnu",("linux","x86_64")=>"x86_64-unknown-linux-gnu",_=>return Err(fail("No standalone binary for this platform"))};
        let name=format!("dohsub-{}",platform);
        let bytes=fetch(asset_url(&release,&name,tag)?)?;
        let checksums=String::from_utf8(fetch(asset_url(&release,"SHA256SUMS",tag)?)?).map_err(|_|fail("Invalid release checksums"))?;
        let checksum=checksums.lines().find_map(|l|{let mut p=l.split_whitespace();let hash=p.next()?;let path=p.next()?.trim_start_matches('*');if path==name{Some(hash)}else{None}}).ok_or_else(||fail("Missing binary checksum"))?;
        let actual=format!("{:x}",Sha256::digest(&bytes));
        if actual!=checksum {return Err(fail("Release checksum mismatch; installation unchanged"));}
        let stage=prefix.join(format!(".dohsub-upgrade-{}",crate::store::random_token()?));fs::create_dir(&stage)?;
        let outcome=(||->Result<()>{
            use std::os::unix::fs::PermissionsExt;
            let binary=stage.join("dohsub");crate::store::write(&binary,&bytes)?;fs::set_permissions(&binary,fs::Permissions::from_mode(0o755))?;
            verify(&binary,tag)?;
            let installer=prefix.join("share/dohsub/install.sh");
            if !installer.is_file(){return Err(fail("Standalone installer is missing"));}
            if !Command::new("bash").arg(installer).arg("--binary").arg(binary).arg("--prefix").arg(prefix).status()?.success(){return Err(fail("Standalone upgrade failed; installation was not replaced"));}
            Ok(())
        })();let _=fs::remove_dir_all(stage);outcome?;
    }
    verify(&destination,tag)?;
    println!("Successfully upgraded dohsub from v{} to {}.",VERSION,tag);io::stdout().flush()?;Ok(())
}
fn verify(binary: &std::path::Path,tag:&str)->Result<()> {
    let out=Command::new(binary).arg("--version").output()?;
    let text=String::from_utf8_lossy(&out.stdout);
    let expected=format!("dohsub version {} (",tag.trim_start_matches('v'));
    if !out.status.success() || text.lines().count()!=2 || !text.starts_with(&expected) || text.lines().nth(1)!=Some(REPOSITORY) {return Err(fail("Installed binary did not report the expected release version"));}Ok(())
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn semver_channels(){assert_eq!(compare("1.9.0","v1.10.0").unwrap(),Channel::Newer);assert_eq!(compare("1.0.0","v1.0.0").unwrap(),Channel::Current);assert_eq!(compare("1.0.0+build.1","v1.0.0").unwrap(),Channel::Current);assert_eq!(compare("2.0.0","v1.9.0").unwrap(),Channel::Ahead);assert_eq!(compare("1.2.3-rc.1","v1.2.3").unwrap(),Channel::Newer);assert!(compare("1.0.0","v2.0.0-rc.1").is_err());assert!(compare("1.0.0","garbage").is_err());}
}
