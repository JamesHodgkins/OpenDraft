//! Finding a running app. Each run writes a descriptor file naming its
//! endpoint and a fresh token into a per-user directory; the sidecar reads
//! it. The file is readable only by the user (mode 0600 on Linux; the
//! per-user profile folder on Windows) and removed on a clean exit. A stale
//! file is discovered by failing to connect, not by guessing from the pid.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Descriptor {
    pub instance_id: String,
    pub pid: u32,
    pub app_version: String,
    pub api_versions: Vec<u32>,
    /// A named pipe name (Windows) or socket path (Linux).
    pub endpoint: String,
    /// 256 random bits as hex. Never logged.
    pub token: String,
}

/// A descriptor without its token, safe to show a model or write to a log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub instance_id: String,
    pub pid: u32,
    pub app_version: String,
    pub api_versions: Vec<u32>,
}

impl Descriptor {
    pub fn info(&self) -> InstanceInfo {
        InstanceInfo {
            instance_id: self.instance_id.clone(),
            pid: self.pid,
            app_version: self.app_version.clone(),
            api_versions: self.api_versions.clone(),
        }
    }
}

/// Names a different descriptor folder, for tests and unusual setups. Both
/// the app and the MCP server read it, so they must be started with the same
/// value.
pub const DIRECTORY_ENV: &str = "OD_AUTOMATION_DIR";

/// The per-user folder holding descriptors, or `None` if the environment
/// does not say where it is.
pub fn directory() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(DIRECTORY_ENV).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)?;
    Some(base.join("OpenDraft").join("automation"))
}

fn file_name(instance_id: &str) -> String {
    format!("{instance_id}.json")
}

/// Only ids this module made are used as file names.
pub fn valid_instance_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

pub fn write(dir: &Path, descriptor: &Descriptor) -> io::Result<PathBuf> {
    if !valid_instance_id(&descriptor.instance_id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bad instance id",
        ));
    }
    fs::create_dir_all(dir)?;
    let path = dir.join(file_name(&descriptor.instance_id));
    let json = serde_json::to_vec(descriptor).map_err(io::Error::other)?;
    write_private(&path, &json)?;
    Ok(path)
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::write(path, bytes)
}

/// Every readable descriptor in `dir`, newest instance ids last. Files that
/// do not parse are skipped.
pub fn list(dir: &Path) -> Vec<Descriptor> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<Descriptor> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| fs::read(e.path()).ok())
        .filter_map(|bytes| serde_json::from_slice::<Descriptor>(&bytes).ok())
        .filter(|d| valid_instance_id(&d.instance_id))
        .collect();
    found.sort_by(|a, b| a.instance_id.cmp(&b.instance_id));
    found
}

/// Remove an instance's descriptor; a missing file is fine.
pub fn remove(dir: &Path, instance_id: &str) {
    if valid_instance_id(instance_id) {
        let _ = fs::remove_file(dir.join(file_name(instance_id)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(id: &str) -> Descriptor {
        Descriptor {
            instance_id: id.to_owned(),
            pid: 1,
            app_version: "0.1.0".to_owned(),
            api_versions: vec![1],
            endpoint: "od-test".to_owned(),
            token: "secret".to_owned(),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("od-api-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_descriptor_is_written_listed_and_removed() {
        let dir = scratch("roundtrip");
        write(&dir, &descriptor("aaa")).unwrap();
        write(&dir, &descriptor("bbb")).unwrap();
        fs::write(dir.join("junk.json"), b"not json").unwrap();
        let found = list(&dir);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0], descriptor("aaa"));
        remove(&dir, "aaa");
        assert_eq!(list(&dir).len(), 1);
        remove(&dir, "aaa");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ids_cannot_escape_the_folder() {
        let dir = scratch("escape");
        for bad in ["", "../x", "a/b", "a.b"] {
            assert!(write(&dir, &descriptor(bad)).is_err(), "{bad:?}");
        }
        remove(&dir, "../x");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_public_info_has_no_token() {
        let json = serde_json::to_string(&descriptor("a").info()).unwrap();
        assert!(!json.contains("secret") && !json.contains("endpoint"));
    }
}
