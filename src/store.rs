use serde::{Deserialize, Serialize};
use std::{fs::{self, File, OpenOptions}, io::{Read, Write}, os::{fd::{AsRawFd, FromRawFd, RawFd}, unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt}}, path::{Path, PathBuf}};
use crate::{Result, fail};

pub const DEFAULT_PORT: u16 = 55355;
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Config { pub doh: Option<String>, pub port: u16, pub interface: Option<String> }
impl Default for Config {
    fn default() -> Self { Self { doh: None, port: DEFAULT_PORT, interface: None } }
}
pub fn home() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("DOHSUB_HOME") { return Ok(PathBuf::from(p)); }
    Ok(PathBuf::from(std::env::var_os("HOME").ok_or_else(|| fail("HOME is not set"))?).join(".dohsub"))
}
pub fn init(dir: &Path) -> Result<()> {
    if fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_symlink()) { return Err(fail("State directory cannot be a symlink")); }
    fs::create_dir_all(dir)?;
    if fs::metadata(dir)?.uid() != unsafe { libc::geteuid() } { return Err(fail("State directory must be owned by you")); }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
pub fn read(path: &Path) -> Result<Vec<u8>> {
    let mut f = OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path)?;
    let m = f.metadata()?;
    if !m.is_file() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 { return Err(fail("Private input must be a regular file owned by you, mode 600 (chmod 600 FILE)")); }
    if m.len() > 1_048_576 { return Err(fail("Input exceeds 1 MiB")); }
    let mut data = Vec::new();
    (&mut f).take(1_048_577).read_to_end(&mut data)?;
    if data.len() > 1_048_576 { return Err(fail("Input exceeds 1 MiB")); }
    Ok(data)
}
pub fn write(path: &Path, data: &[u8]) -> Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) { return Err(fail("Refusing to replace a symlink")); }
    let parent = path.parent().ok_or_else(|| fail("Invalid state path"))?;
    let temp = parent.join(format!(".write-{}-{}", std::process::id(), random_token()?));
    let result = (|| -> Result<()> {
        let mut f = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temp)?;
        f.write_all(data)?; f.sync_all()?;
        fs::rename(&temp, path)?; Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temp); }
    result
}
pub fn load<T: for<'a> Deserialize<'a> + Default>(path: &Path) -> Result<T> {
    if !path.exists() { return Ok(T::default()); }
    serde_json::from_slice(&read(path)?).map_err(|_| fail("Invalid private state file; no private contents were logged"))
}
pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let data=serde_json::to_vec_pretty(value)?;
    if data.len()>1_048_576 {return Err(fail("Private state would exceed 1 MiB; existing file unchanged"));}
    write(path,&data)
}
pub fn random_token() -> Result<String> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let mut data = [0u8; 32]; File::open("/dev/urandom")?.read_exact(&mut data)?;
    Ok(URL_SAFE_NO_PAD.encode(data))
}
pub fn token(dir: &Path) -> Result<String> {
    let p = dir.join("token");
    if !p.exists() { write(&p, random_token()?.as_bytes())?; }
    let s = String::from_utf8(read(&p)?).map_err(|_| fail("Invalid subscription token"))?;
    if s.len() != 43 || !s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-') { return Err(fail("Invalid subscription token")); }
    Ok(s)
}
pub struct Lock(File);
impl Lock {
    pub fn acquire(path: &Path, blocking: bool) -> Result<Self> {
        let f = OpenOptions::new().create(true).truncate(false).read(true).write(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(path)?;
        let m = f.metadata()?;
        if !m.is_file() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 { return Err(fail("Invalid private lock file")); }
        let op = libc::LOCK_EX | if blocking { 0 } else { libc::LOCK_NB };
        if unsafe { libc::flock(f.as_raw_fd(), op) } != 0 { return Err(fail("dohsub is already running or another operation is in progress")); }
        Ok(Self(f))
    }
    pub fn fd(&self) -> RawFd { self.0.as_raw_fd() }
    pub unsafe fn inherited(fd: RawFd) -> Self { Self(unsafe { File::from_raw_fd(fd) }) }
}
pub fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
